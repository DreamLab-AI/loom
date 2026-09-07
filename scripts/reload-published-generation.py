#!/usr/bin/env python3
"""Reload only a complete, hash-verified published bundle; receipt after served proof.

Run once from an operator-installed timer. This does not download or promote data,
change deployment configuration, or construct a semantic index.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
import urllib.request

REQUIRED = frozenset(('scaffold-index.json', 'prose-index.json', 'ontology.ttl',
                     'ontology-inferred.ttl', 'ontology-corpus.rvdb',
                     'ontology-corpus.rvdb.generation.json'))


def verified_bundle(directory):
    directory = Path(directory)
    marker = directory / '.generation.json'
    if (directory / '.promotion-in-flight').exists() or marker.is_symlink():
        raise ValueError('publication in flight or indirect marker')
    raw = marker.read_bytes()
    manifest = json.loads(raw)
    generation = manifest.get('generation')
    artifacts = manifest.get('artifacts', {})
    if not isinstance(generation, str) or not generation or not REQUIRED <= artifacts.keys():
        raise ValueError('complete graph and semantic generation required')
    digests = []
    for name, spec in artifacts.items():
        if Path(name).name != name or name in ('.', '..'):
            raise ValueError('artifact must be a direct filename')
        artifact = directory / name
        if artifact.is_symlink() or not artifact.is_file():
            raise ValueError('artifact missing or indirect')
        with artifact.open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        if digest != spec.get('sha256') or artifact.stat().st_size != spec.get('bytes'):
            raise ValueError('artifact hash or size mismatch: ' + name)
        digests.append(f'{name}:{digest}')
    sidecar = json.loads((directory / 'ontology-corpus.rvdb.generation.json').read_bytes())
    if sidecar.get('generatedAt') != generation or sidecar.get('embeddingModel') != 'bge-small-en-v1.5' or sidecar.get('dimensions') != 384:
        raise ValueError('semantic sidecar must match generation/model/dimensions')
    if marker.read_bytes() != raw or (directory / '.promotion-in-flight').exists():
        raise ValueError('publication changed during verification')
    return {'generation': generation,
            'content_digest': hashlib.sha256('\n'.join(sorted(digests)).encode()).hexdigest()}


def served_matches(report, bundle):
    identity = report.get('identity', {})
    embedding = report.get('embedding', {})
    return (report.get('id') == bundle['generation']
            and identity.get('generation', {}).get('id') == bundle['generation']
            and identity.get('content_digest') == bundle['content_digest']
            and identity.get('atomicity_verified') is True
            and report.get('disk', {}).get('matches_loaded') is True
            and report.get('drift', {}).get('ok') is True
            and report.get('semantic_generation', {}).get('id') == bundle['generation']
            and embedding.get('model_id') == 'bge-small-en-v1.5'
            and embedding.get('dimensions') == 384 and embedding.get('metric') == 'cosine'
            and embedding.get('rejections') == [])


def write_receipt(path, value):
    path = Path(path)
    temp = path.with_suffix(path.suffix + '.tmp')
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, 'w') as stream:
        json.dump(value, stream, sort_keys=True)
        stream.write('\n')
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temp, path)


def reconcile(directory, receipt, fetch, reload, timeout=60, interval=1):
    bundle = verified_bundle(directory)
    try:
        if served_matches(fetch(), bundle):
            write_receipt(receipt, {'state': 'served', **bundle})
            return 'current'
    except (OSError, ValueError):
        pass
    # Failed attempts are never persisted as successful; retries remain possible.
    write_receipt(receipt, {'state': 'reload-pending', **bundle})
    reload()
    deadline = time.monotonic() + timeout
    while True:
        # Refuse a concurrent publication rather than acknowledging the wrong one.
        if verified_bundle(directory) != bundle:
            raise ValueError('publication changed during reload')
        try:
            if served_matches(fetch(), bundle):
                write_receipt(receipt, {'state': 'served', **bundle})
                return 'reloaded'
        except (OSError, ValueError):
            pass
        if time.monotonic() >= deadline:
            raise TimeoutError('reloaded service did not prove the expected bundle')
        time.sleep(interval)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data-dir', required=True)
    parser.add_argument('--receipt', required=True)
    parser.add_argument('--url', required=True)
    parser.add_argument('--timeout', type=int, default=60)
    parser.add_argument('reload_command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.reload_command
    if command and command[0] == '--': command = command[1:]
    if not command or not 1 <= args.timeout <= 300:
        parser.error('a reload command and timeout 1..300 are required')
    receipt = Path(args.receipt)
    receipt.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    with open(str(receipt) + '.lock', 'a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        def fetch():
            with urllib.request.urlopen(args.url.rstrip('/') + '/loom/generation', timeout=5) as response:
                raw = response.read(1_048_577)
            if len(raw) > 1_048_576: raise ValueError('oversized identity response')
            return json.loads(raw)
        def reload():
            subprocess.run(command, check=True, timeout=args.timeout)
        print(reconcile(args.data_dir, receipt, fetch, reload, args.timeout))

if __name__ == '__main__':
    main()
