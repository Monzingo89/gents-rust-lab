#!/usr/bin/env python3
"""Acquire a pinned speech archive and copy only bounded, selected regular members."""
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import ssl
import tarfile
import urllib.request
import wave

import certifi

ROOT = Path(__file__).resolve().parents[1]
URL = 'https://data.keithito.com/data/speech/LJSpeech-1.1.tar.bz2'
SIZE = 2748572632
SHA = 'be1a30453f28eb8dd26af4101ae40cbf2c50413b1bb21936cbcdc6fae3de8aa5'
CHECKSUM_SOURCE = 'https://raw.githubusercontent.com/tensorflow/datasets/master/tensorflow_datasets/datasets/ljspeech/checksums.tsv'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def bounded_member(archive, member, limit):
    if not member.isfile() or member.size <= 0 or member.size > limit:
        raise ValueError('member type or size rejected')
    stream = archive.extractfile(member)
    if stream is None:
        raise ValueError('missing member data')
    payload = stream.read(limit + 1)
    if len(payload) != member.size:
        raise ValueError('member size mismatch')
    return payload


def select_records(payload):
    records = {}
    for line in payload.decode('utf-8').splitlines():
        identifier, _, text = line.split('|', 2)
        if not re.fullmatch(r'LJ\d{3}-\d{4}', identifier) or identifier in records:
            raise ValueError('invalid or duplicate clip identifier')
        if not text.strip() or len(text) > 4096:
            raise ValueError('invalid transcript')
        records[identifier] = text
    ordered = sorted(records, key=lambda key: hashlib.sha256(('audio-weight-v1:' + key).encode()).digest())
    if len(ordered) < 64:
        raise ValueError('insufficient paired records')
    selected = {}
    for index, identifier in enumerate(ordered[:64]):
        selected[identifier] = {'id': identifier, 'text': records[identifier],
            'split': 'train' if index < 48 else 'validation' if index < 56 else 'test'}
    if len({entry['text'] for entry in selected.values()}) != 64:
        raise ValueError('duplicate selected transcripts')
    return selected


def main():
    os.umask(0o077)
    target = ROOT / 'data/weight-experiments/audio-paired-v1'
    target.mkdir(parents=True, exist_ok=True)
    archive_path = target / 'LJSpeech-1.1.tar.bz2'
    if not archive_path.exists():
        if shutil.disk_usage(target).free < 4 * 1024**3:
            raise ValueError('insufficient free space')
        partial = target / 'archive.partial'
        context = ssl.create_default_context(cafile=certifi.where())
        count = 0
        with urllib.request.urlopen(URL, context=context, timeout=60) as response, partial.open('xb') as output:
            if not response.url.startswith('https://'):
                raise ValueError('non-TLS redirect')
            while block := response.read(1024 * 1024):
                count += len(block)
                if count > SIZE:
                    raise ValueError('archive exceeds pinned size')
                output.write(block)
            output.flush()
            os.fsync(output.fileno())
        if count != SIZE or digest(partial) != SHA:
            raise ValueError('archive pin mismatch')
        partial.rename(archive_path)
    if archive_path.is_symlink() or archive_path.stat().st_size != SIZE or digest(archive_path) != SHA:
        raise ValueError('archive pin mismatch')
    print('Archive verified; preparing fixed splits.', flush=True)
    destination = target / 'paired'
    destination.mkdir(exist_ok=False)
    # Metadata may occur after audio. Two streaming passes avoid trusting tar paths
    # or retaining an unbounded decompressed archive/member index.
    selected = None
    with tarfile.open(archive_path, 'r|bz2') as archive:
        for member in archive:
            if member.name == 'LJSpeech-1.1/metadata.csv':
                selected = select_records(bounded_member(archive, member, 8 * 1024**2))
                break
    if selected is None:
        raise ValueError('metadata missing')
    found = set()
    with tarfile.open(archive_path, 'r|bz2') as archive:
        for member in archive:
            match = re.fullmatch(r'LJSpeech-1.1/wavs/(LJ\d{3}-\d{4})\.wav', member.name)
            if not match or match[1] not in selected:
                continue
            identifier = match[1]
            if identifier in found:
                raise ValueError('duplicate audio member')
            payload = bounded_member(archive, member, 2 * 1024**2)
            with wave.open(io.BytesIO(payload)) as audio:
                if (audio.getnchannels(), audio.getsampwidth(), audio.getframerate(), audio.getcomptype()) != (1, 2, 22050, 'NONE'):
                    raise ValueError('unexpected audio format')
                frames = audio.getnframes()
                if not 0 < frames <= 22050 * 20 or len(audio.readframes(frames)) != frames * 2:
                    raise ValueError('invalid audio duration or frames')
            with (destination / (identifier + '.wav')).open('xb') as output:
                output.write(payload)
            selected[identifier].update(sha256=hashlib.sha256(payload).hexdigest(), samples=frames)
            found.add(identifier)
    if found != set(selected):
        raise ValueError('missing selected audio')
    report = {'archive_sha256': SHA, 'archive_bytes': SIZE, 'source': URL,
        'checksum_source': CHECKSUM_SOURCE, 'selection': 'sha256(audio-weight-v1:clip-id), first 64; 48/8/8',
        'pretraining_overlap': 'Likely: this voice was trained on LJ Speech. These are fine-tuning splits, not unseen pretraining data.',
        'provider_cost_usd': 0, 'weights_changed': False, 'records': list(selected.values())}
    (destination / 'manifest.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'prepared': len(found), 'train': 48, 'validation': 8, 'test': 8}), flush=True)


if __name__ == '__main__':
    main()
