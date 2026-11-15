import io
import tarfile
import unittest

from prepare_audio_weight_dataset import bounded_member, select_records


class AudioDatasetTests(unittest.TestCase):
    def test_fixed_disjoint_splits(self):
        payload = '\n'.join(f'LJ001-{i:04d}|raw|Sentence {i}.' for i in range(100)).encode()
        selected = select_records(payload)
        self.assertEqual(selected, select_records(payload))
        self.assertEqual([sum(r['split'] == split for r in selected.values())
                          for split in ('train', 'validation', 'test')], [48, 8, 8])
        self.assertEqual(len(selected), 64)
        with self.assertRaises(ValueError):
            select_records(payload + b'\nLJ001-0000|raw|Duplicate identifier.')

    def test_reject_links_and_oversized_members(self):
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode='w') as archive:
            member = tarfile.TarInfo('linked.wav')
            member.type = tarfile.SYMTYPE
            member.linkname = '/etc/passwd'
            archive.addfile(member)
            member = tarfile.TarInfo('oversized.wav')
            member.size = 10
            archive.addfile(member, io.BytesIO(b'x' * 10))
        buffer.seek(0)
        with tarfile.open(fileobj=buffer) as archive:
            for member in archive:
                with self.assertRaises(ValueError):
                    bounded_member(archive, member, 4)


if __name__ == '__main__':
    unittest.main()
