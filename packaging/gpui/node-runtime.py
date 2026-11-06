#!/usr/bin/env python3
"""Download one pinned official Node distribution and verify its archive digest.

Build-time only. The installed app never calls this script or downloads a runtime.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import tarfile
import urllib.request
import zipfile


def download(url, path):
    with urllib.request.urlopen(url, timeout=120) as response, path.open('wb') as out:
        while chunk := response.read(1024 * 1024):
            out.write(chunk)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--version', default='24.22.2')
    parser.add_argument('--platform', required=True, choices=['linux', 'darwin', 'win'])
    parser.add_argument('--arch', required=True, choices=['x64', 'arm64'])
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    if not all(p.isdecimal() for p in args.version.split('.')) or len(args.version.split('.')) != 3:
        raise ValueError('Expected exact Node semver')
    root = Path(args.output).resolve()
    root.mkdir(parents=True, exist_ok=True)
    name = f'node-v{args.version}-{args.platform}-{args.arch}'
    archive_name = name + ('.zip' if args.platform == 'win' else '.tar.gz')
    base = f'https://nodejs.org/dist/v{args.version}/'
    checksum_file = root / 'SHASUMS256.txt'
    archive = root / archive_name
    download(base + 'SHASUMS256.txt', checksum_file)
    matches = [line.split()[0] for line in checksum_file.read_text().splitlines() if line.split()[-1] == archive_name]
    if len(matches) != 1 or len(matches[0]) != 64:
        raise ValueError('Official checksum entry is missing or ambiguous')
    download(base + archive_name, archive)
    actual = hashlib.file_digest(archive.open('rb'), 'sha256').hexdigest()
    if actual != matches[0]:
        archive.unlink()
        raise ValueError('Official Node archive checksum mismatch')
    # Extract only the executable and its exact matching license, never arbitrary
    # paths, symlinks, npm, or shell launchers from the archive.
    binary_name = 'node.exe' if args.platform == 'win' else 'bin/node'
    wanted = {f'{name}/{binary_name}': root / ('node.exe' if args.platform == 'win' else 'node'), f'{name}/LICENSE': root / 'LICENSE'}
    if args.platform == 'win':
        with zipfile.ZipFile(archive) as package:
            for member, dest in wanted.items():
                dest.write_bytes(package.read(member))
    else:
        with tarfile.open(archive, 'r:gz') as package:
            for member, dest in wanted.items():
                entry = package.getmember(member)
                if not entry.isfile():
                    raise ValueError(f'Expected regular file: {member}')
                dest.write_bytes(package.extractfile(entry).read())
    node = wanted[f'{name}/{binary_name}']
    node.chmod(0o755)
    receipt = {'version': args.version, 'platform': args.platform, 'arch': args.arch, 'url': base + archive_name, 'archive_sha256': actual, 'executable_sha256': hashlib.file_digest(node.open('rb'), 'sha256').hexdigest()}
    (root / 'provenance.json').write_text(json.dumps(receipt, indent=2) + '\n')
    archive.unlink()
    print(json.dumps(receipt))


if __name__ == '__main__':
    main()
