#!/usr/bin/env python3
"""Verify retained security backports before one narrowly scoped audit exception."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]


def verify_backports(root=ROOT):
    vendor=root/'src-tauri/vendor'
    pins=json.loads((vendor/'provenance.json').read_text())
    for name,pin in pins.items():
        base=vendor/name
        expected={**pin['original_sha256'],**pin['modified_sha256']}
        actual={str(p.relative_to(base)):p for p in base.rglob('*') if p.is_file() and p.name not in ('Cargo.lock',)}
        if set(actual)!=set(expected):raise ValueError('unexpected vendored file inventory: '+name)
        for relative,path in actual.items():
            if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest()!=expected[relative]:
                raise ValueError('vendored source hash mismatch: '+name+'/'+relative)
    source=(vendor/'glib-0.18.5/src/variant_iter.rs').read_text()
    if 'let mut p: *mut libc::c_char = std::ptr::null_mut();' not in source or '                &mut p,' not in source:
        raise ValueError('required GLib out-pointer backport missing')
    manifest=(root/'src-tauri/Cargo.toml').read_text()
    for name in pins:
        if 'vendor/'+name not in manifest:raise ValueError('backport not patched into desktop graph')


def main():
    verify_backports()
    result=subprocess.run(['cargo','audit','--deny','warnings','--ignore','RUSTSEC-2024-0429','--file',str(ROOT/'src-tauri/Cargo.lock'),'--json'],capture_output=True,text=True)
    if result.returncode:
        print('Desktop dependency audit failed; inspect cargo audit output locally.',file=sys.stderr)
        return result.returncode
    audit=json.loads(result.stdout)
    print(json.dumps({'status':'PASS','vulnerabilities':audit['vulnerabilities']['count'],'backport_source_verified':True,
        'version_advisory_exception':'RUSTSEC-2024-0429: exact upstream two-line fix is retained and hash verified; original package version preserved',
        'other_advisories_ignored':False}))
    return 0


if __name__=='__main__':sys.exit(main())
