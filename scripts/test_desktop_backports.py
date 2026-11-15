import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

SCRIPT=Path(__file__).resolve().with_name('audit-desktop-dependencies.py')
spec=importlib.util.spec_from_file_location('desktop_audit',SCRIPT)
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)


class BackportTests(unittest.TestCase):
    def test_missing_backport_or_unlisted_file_is_rejected(self):
        module.verify_backports()
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            shutil.copytree(module.ROOT/'src-tauri',root/'src-tauri',ignore=shutil.ignore_patterns('target','Cargo.lock','icons','src','gen','capabilities','build.rs','tauri.conf.json'))
            # The top-level src exclusion also omitted vendor src; copy it explicitly.
            shutil.rmtree(root/'src-tauri/vendor')
            shutil.copytree(module.ROOT/'src-tauri/vendor',root/'src-tauri/vendor',ignore=shutil.ignore_patterns('Cargo.lock'))
            module.verify_backports(root)
            path=root/'src-tauri/vendor/glib-0.18.5/src/variant_iter.rs'
            original=path.read_text();path.write_text(original.replace('&mut p,','&p,'))
            with self.assertRaises(ValueError):module.verify_backports(root)
            path.write_text(original)
            (path.parent/'unexpected.rs').write_text('// unreviewed')
            with self.assertRaises(ValueError):module.verify_backports(root)


if __name__=='__main__':unittest.main()
