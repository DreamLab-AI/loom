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

if __name__ == '__main__': unittest.main()
