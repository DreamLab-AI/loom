import importlib.util
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('reload_generation', Path(__file__).parents[1] / 'scripts/reload-published-generation.py')
reload = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reload)

class ReloadTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.receipt = self.root / 'receipt.json'
        self.manifest = {'generation': 'g1', 'artifacts': {}}
        for name in reload.REQUIRED:
            content = b'fixture'
            if name.endswith('.generation.json'):
                content = json.dumps({'generatedAt': 'g1', 'embeddingModel': 'bge-small-en-v1.5', 'dimensions': 384}).encode()
            (self.root / name).write_bytes(content)
            self.manifest['artifacts'][name] = {'sha256': hashlib.sha256(content).hexdigest(), 'bytes': len(content)}
        self.save()
    def save(self):
        (self.root / '.generation.json').write_text(json.dumps(self.manifest))
    def serving(self):
        bundle = reload.verified_bundle(self.root)
        return {'id': 'g1', 'identity': {'generation': {'id': 'g1'}, 'content_digest': bundle['content_digest'], 'atomicity_verified': True}, 'disk': {'matches_loaded': True}, 'drift': {'ok': True}, 'semantic_generation': {'id': 'g1'}, 'embedding': {'model_id': 'bge-small-en-v1.5', 'dimensions': 384, 'metric': 'cosine', 'rejections': []}}
    def test_matching_server_does_not_restart(self):
        self.assertEqual(reload.reconcile(self.root, self.receipt, self.serving, lambda: self.fail('unexpected restart')), 'current')
    def test_new_publication_reloads_then_records_verified_receipt(self):
        state = {'restarted': False}
        def restart(): state['restarted'] = True
        def fetch(): return self.serving() if state['restarted'] else {}
        self.assertEqual(reload.reconcile(self.root, self.receipt, fetch, restart), 'reloaded')
        self.assertEqual(json.loads(self.receipt.read_text())['state'], 'served')
    def test_failed_reload_never_records_success(self):
        def restart(): raise RuntimeError('restart failed')
        with self.assertRaises(RuntimeError): reload.reconcile(self.root, self.receipt, lambda: {}, restart)
        self.assertEqual(json.loads(self.receipt.read_text())['state'], 'reload-pending')
    def test_wrong_generation_after_restart_is_not_acknowledged(self):
        with self.assertRaises(TimeoutError): reload.reconcile(self.root, self.receipt, lambda: {}, lambda: None, timeout=0)
        self.assertEqual(json.loads(self.receipt.read_text())['state'], 'reload-pending')
    def test_hash_tamper_prevents_restart(self):
        (self.root / 'ontology.ttl').write_text('tampered')
        with self.assertRaises(ValueError): reload.reconcile(self.root, self.receipt, lambda: {}, lambda: self.fail('restart'))
        self.assertFalse(self.receipt.exists())
    def test_missing_semantic_and_inflight_publication_refused(self):
        del self.manifest['artifacts']['ontology-corpus.rvdb']; self.save()
        with self.assertRaises(ValueError): reload.verified_bundle(self.root)
        (self.root / '.promotion-in-flight').touch()
        with self.assertRaises(ValueError): reload.verified_bundle(self.root)
    def test_indirect_and_traversal_artifacts_refused(self):
        self.manifest['artifacts']['../escape'] = {'sha256': 'x', 'bytes': 0}; self.save()
        with self.assertRaises(ValueError): reload.verified_bundle(self.root)
    def test_concurrent_publication_after_restart_not_acknowledged(self):
        def restart(): (self.root / '.promotion-in-flight').touch()
        with self.assertRaises(ValueError): reload.reconcile(self.root, self.receipt, lambda: {}, restart)
        self.assertEqual(json.loads(self.receipt.read_text())['state'], 'reload-pending')

class VaultBuildMarkerTests(unittest.TestCase):
    """The `vault build` commit marker (contract C3) — the shape Loom serves after
    the first clean promotion. The legacy mirror shape stays covered above: the
    reload script, like the node, reads both."""
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.receipt = self.root / 'receipt.json'
        self.identity = 'visionGraph@abc1234'
        artifacts, digests = [], []
        for name in sorted(reload.REQUIRED):
            content = b'fixture'
            if name.endswith('.generation.json'):
                content = json.dumps({'generatedAt': self.identity, 'embeddingModel': 'bge-small-en-v1.5', 'dimensions': 384}).encode()
            (self.root / name).write_bytes(content)
            digest = hashlib.sha256(content).hexdigest()
            artifacts.append({'name': name, 'sha256': digest, 'bytes': len(content)})
            digests.append(f'{name}:{digest}')
        self.manifest = {'id': self.identity, 'commit': 'abc1234',
                         'content_digest': hashlib.sha256('\n'.join(sorted(digests)).encode()).hexdigest(),
                         'generated_at': '2026-09-22T00:00:00Z', 'class_count': 8146,
                         'page_count': 8454, 'vocabulary_version': 1,
                         'stale_after': '2026-10-06', 'artifacts': artifacts}
        self.save()
    def save(self):
        (self.root / '.generation.json').write_text(json.dumps(self.manifest))
    def test_graph_tier_subdirectory_artifact_is_accepted(self):
        (self.root / 'graph').mkdir()
        content = b'tier'
        (self.root / 'graph' / 'full.bin').write_bytes(content)
        digest = hashlib.sha256(content).hexdigest()
        self.manifest['artifacts'].append({'name': 'graph/full.bin', 'sha256': digest, 'bytes': len(content)})
        digests = [f"{a['name']}:{a['sha256']}" for a in self.manifest['artifacts']]
        self.manifest['content_digest'] = hashlib.sha256('\n'.join(sorted(digests)).encode()).hexdigest()
        self.save()
        self.assertEqual(reload.verified_bundle(self.root)['content_digest'], self.manifest['content_digest'])
    def test_escaping_or_indirect_subpaths_are_refused(self):
        outside = Path(self.temp.name).parent / 'outside.bin'
        (self.root / 'graph').mkdir()
        (self.root / 'linked').symlink_to(self.root / 'graph')
        for name in ('graph/../../outside.bin', '/etc/passwd', 'graph//full.bin', './graph/full.bin',
                     'graph\\full.bin', 'linked/full.bin'):
            with self.subTest(name=name):
                self.setUp_marker_with(name)
                with self.assertRaises(ValueError): reload.verified_bundle(self.root)
    def setUp_marker_with(self, name):
        self.manifest['artifacts'] = [a for a in self.manifest['artifacts'] if a['name'] in reload.REQUIRED]
        self.manifest['artifacts'].append({'name': name, 'sha256': '0' * 64, 'bytes': 0})
        self.save()
    def test_vault_build_bundle_is_accepted(self):
        bundle = reload.verified_bundle(self.root)
        self.assertEqual(bundle['generation'], self.identity)
        self.assertEqual(bundle['content_digest'], self.manifest['content_digest'])
    def test_declared_corpus_digest_is_carried_not_compared(self):
        # `content_digest` in a vault-build marker is the source-corpus digest;
        # it cannot be recomputed from the artefacts and must not be.
        self.manifest['content_digest'] = '0' * 64; self.save()
        reload.verified_bundle(self.root)
    def test_a_tampered_artifact_still_refuses_whatever_the_declared_digest(self):
        (self.root / 'ontology.ttl').write_bytes(b'tampered'); self.save()
        with self.assertRaises(ValueError): reload.verified_bundle(self.root)
    def test_identity_must_name_its_own_commit(self):
        self.manifest['commit'] = 'deadbee'; self.save()
        with self.assertRaises(ValueError): reload.verified_bundle(self.root)
    def test_missing_c3_field_is_refused(self):
        del self.manifest['vocabulary_version']; self.save()
        with self.assertRaises(ValueError): reload.verified_bundle(self.root)
    def test_semantic_sidecar_must_declare_the_same_generation(self):
        # Re-stamp the marker too, so the ONLY disagreement under test is the
        # sidecar's generation rather than its hash.
        name = 'ontology-corpus.rvdb.generation.json'
        content = json.dumps({'generatedAt': 'visionGraph@other', 'embeddingModel': 'bge-small-en-v1.5', 'dimensions': 384}).encode()
        (self.root / name).write_bytes(content)
        digests = []
        for spec in self.manifest['artifacts']:
            if spec['name'] == name:
                spec['sha256'], spec['bytes'] = hashlib.sha256(content).hexdigest(), len(content)
            digests.append(f"{spec['name']}:{spec['sha256']}")
        self.manifest['content_digest'] = hashlib.sha256('\n'.join(sorted(digests)).encode()).hexdigest()
        self.save()
        with self.assertRaises(ValueError): reload.verified_bundle(self.root)


if __name__ == '__main__': unittest.main()
