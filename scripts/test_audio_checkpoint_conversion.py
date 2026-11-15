import hashlib
from pathlib import Path
import tempfile
import unittest
from audio_checkpoint_conversion import convert


class Unsupported:
    def __reduce__(self):return (eval,('1+1',))


class ConversionTests(unittest.TestCase):
    def test_valid_tensor_conversion_and_rejection_paths(self):
        import torch
        from safetensors.torch import load_file
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);source=root/'input.ckpt'
            torch.save({'state_dict':{'weight':torch.tensor([1.,2.])},'path':Path('fixture')},source)
            digest=hashlib.sha256(source.read_bytes()).hexdigest()
            report=convert(source,root/'good',digest)
            self.assertTrue(report['weights_only'])
            self.assertTrue(torch.equal(load_file(root/'good/state.safetensors')['weight'],torch.tensor([1.,2.])))
            with self.assertRaisesRegex(ValueError,'hash mismatch'):convert(source,root/'bad','0'*64)
            torch.save({'state_dict':{},'payload':Unsupported()},source)
            with self.assertRaisesRegex(ValueError,'unapproved pickle globals'):
                convert(source,root/'unsafe',hashlib.sha256(source.read_bytes()).hexdigest())
            self.assertFalse((root/'unsafe').exists())
