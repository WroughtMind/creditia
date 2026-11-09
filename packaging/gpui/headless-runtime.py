#!/usr/bin/env python3
"""Retain a compiled native pixel harness and its Linux software GPU runtime.

No compiler, Cargo registry or system installation is included. The resulting
bundle is for local diagnosis of the exact CI-built executable; normal platform
and installation acceptance still runs independently in the workflow.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess


def sha(file):
    with file.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def command(args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT, timeout=30)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--build-json', required=True)
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    executable = None
    for line in Path(args.build_json).read_text().splitlines():
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        if record.get('reason') == 'compiler-artifact' and record.get('target', {}).get('name') == 'rendering' and record.get('executable'):
            executable = Path(record['executable'])
    if executable is None or not executable.is_file():
        raise RuntimeError('Cargo did not identify a compiled rendering harness')
    root = Path(args.output).resolve()
    if root.exists() and any(root.iterdir()):
        raise RuntimeError('Use an empty diagnostic runtime destination')
    for directory in ('bin', 'lib', 'icd', 'licenses'):
        (root / directory).mkdir(parents=True, exist_ok=True)
    shutil.copy2(executable, root / 'bin/rendering')
    manifests = sorted(Path('/usr/share/vulkan/icd.d').glob('*lvp*.json'))
    if len(manifests) != 1:
        raise RuntimeError(f'Expected one installed lavapipe ICD, found {manifests}')
    icd = json.loads(manifests[0].read_text())
    cache = command(['ldconfig', '-p'])

    def resolve(name):
        path = Path(name)
        if path.is_absolute() and path.is_file():
            return path
        for row in cache.splitlines():
            match = re.match(r'\s*(\S+)\s+.*=>\s+(/\S+)', row)
            if match and match[1] == path.name and 'x86-64' in row:
                return Path(match[2])
        fallback = Path('/usr/lib/x86_64-linux-gnu') / path.name
        if fallback.is_file():
            return fallback
        raise RuntimeError(f'Cannot resolve GPU runtime library {name}')

    driver = resolve(icd['ICD']['library_path'])
    loader = resolve('libvulkan.so.1')
    origins = {driver.name: driver, loader.name: loader}
    system = set()
    glibc = re.compile(r'^(?:ld-linux|lib(?:c|m|pthread|rt|dl|util|anl)\.so)')
    for binary in (executable, driver, loader):
        linked = command(['ldd', str(binary)])
        if 'not found' in linked:
            raise RuntimeError(f'Unresolved native dependency: {linked}')
        for soname, source in re.findall(r'\s*(\S+)\s+=>\s+(/[^\s]+)', linked):
            if glibc.match(soname):
                system.add(source)
            else:
                origins[soname] = Path(source)
    libraries, packages = [], set()
    for soname, source in sorted(origins.items()):
        target = root / 'lib' / soname
        shutil.copyfile(source.resolve(), target)
        libraries.append({'path': f'lib/{soname}', 'source': str(source), 'sha256': sha(target), 'bytes': target.stat().st_size})
        # Keep the license/copyright records for these diagnostic dependencies.
        result = subprocess.run(['dpkg-query', '-S', str(source)], capture_output=True, text=True, timeout=10)
        if result.returncode:
            result = subprocess.run(['dpkg-query', '-S', str(source.resolve())], capture_output=True, text=True, timeout=10)
        for line in result.stdout.splitlines():
            if ': /' in line:
                packages.add(line.split(': /', 1)[0].split(':', 1)[0])
    for package in sorted(packages):
        license_file = Path('/usr/share/doc') / package / 'copyright'
        if license_file.is_file():
            shutil.copyfile(license_file, root / 'licenses' / f'{package}.txt')
    icd['ICD']['library_path'] = '../lib/' + driver.name
    (root / 'icd/lvp.json').write_text(json.dumps(icd, indent=2) + '\n')
    launcher = root / 'run'
    launcher.write_text('''#!/bin/sh
set -eu
runtime_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export LD_LIBRARY_PATH="$runtime_root/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export VK_ICD_FILENAMES="$runtime_root/icd/lvp.json"
exec "$runtime_root/bin/rendering" "$@"
''')
    launcher.chmod(0o755)
    (root / 'README.txt').write_text('''Exact CI-compiled GPUI rendering harness with Mesa lavapipe; no Rust compiler or installation required.
Requires compatible Linux x86_64/glibc2.39 and source UI font assets.
Set CREDITIA_RENDER_BRIDGE to the matching checkout packages/render/tools/bridge.mjs.
Set CREDITIA_VISUAL_FIXTURES to canonical fixtures generated by that checkout and CREDITIA_VISUAL_OUTPUT to a fresh output directory, then run ./run.
For catalog mode set CREDITIA_NATIVE_CATALOG and CREDITIA_NATIVE_CATALOG_CAPTURE instead.
The provenance file identifies the compiled candidate and retained library hashes. Local diagnostic captures do not replace the CI platform/installer acceptance gates.
''')
    provenance = {'schema_version': 1, 'candidate_sha': os.environ.get('GITHUB_SHA'), 'harness_sha256': sha(root / 'bin/rendering'),
                  'cargo_artifact_path': str(executable), 'libraries': libraries, 'system_libraries': sorted(system),
                  'source_icd': str(manifests[0]), 'license_packages': sorted(packages)}
    (root / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
    print(json.dumps({'libraries': len(libraries), 'harness_bytes': (root / 'bin/rendering').stat().st_size,
                      'library_bytes': sum(item['bytes'] for item in libraries), 'candidate_sha': provenance['candidate_sha']}))


if __name__ == '__main__':
    main()
