"""Exercise startup contracts without loading model weights or using a GPU."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class ServingContract(unittest.TestCase):
    def run_profile(self, vision='1', spec='off', projector=True):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            for name in ['model.gguf', 'draft.gguf', 'template.jinja']:
                (root / name).touch()
            if projector:
                (root / 'projector.gguf').touch()
            binary = root / 'llama-server'
            binary.write_text('#!/bin/sh\nprintf "%s\\n" "$@"\n')
            binary.chmod(0o755)
            env = dict(os.environ, LLAMA_SERVER=str(binary), MAIN_GGUF=str(root / 'model.gguf'),
                       MMPROJ=str(root / 'projector.gguf'), DRAFT_GGUF=str(root / 'draft.gguf'),
                       CHAT_TEMPLATE_FILE=str(root / 'template.jinja'), VISION=vision, SPEC=spec)
            return subprocess.run(['bash', str(Path(__file__).with_name('serve-qwen38.sh'))],
                                  env=env, text=True, capture_output=True)

    def test_vision_loads_projector_and_template_without_draft(self):
        r = self.run_profile()
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn('--mmproj\n', r.stdout)
        self.assertIn('--chat-template-file\n', r.stdout)
        self.assertNotIn('--spec-type\n', r.stdout)

    def test_missing_projector_cannot_silently_disable_vision(self):
        r = self.run_profile(projector=False)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn('vision requested but projector not found', r.stderr)

    def test_incompatible_speculation_fails_before_startup(self):
        r = self.run_profile(spec='dflash')
        self.assertNotEqual(r.returncode, 0)
        self.assertIn('SPEC=dflash with VISION=1', r.stderr)

    def test_text_profile_still_supports_dflash(self):
        r = self.run_profile(vision='0', spec='dflash', projector=False)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn('draft-dflash\n', r.stdout)
        self.assertNotIn('--mmproj\n', r.stdout)


if __name__ == '__main__':
    unittest.main()
