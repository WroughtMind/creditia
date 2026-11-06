#!/usr/bin/env python3
"""Retain only rustfmt and its toolchain-local dynamic dependencies for review."""
import argparse
import json
import pathlib
import re
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--output', required=True)
args = parser.parse_args()
output = pathlib.Path(args.output).resolve()
binary = pathlib.Path(subprocess.check_output(['rustup', 'which', 'rustfmt'], text=True).strip()).resolve()
toolchain = binary.parent.parent
(output / 'bin').mkdir(parents=True, exist_ok=True)
shutil.copy2(binary, output / 'bin' / 'rustfmt')
pending, copied, system = [binary], set(), set()
while pending:
    current = pending.pop()
    linked = subprocess.check_output(['ldd', str(current)], text=True)
    for raw in re.findall(r'(?:=>\s+)?(/[^\s]+)', linked):
        dependency = pathlib.Path(raw)
        try:
            relative = dependency.relative_to(toolchain)
        except ValueError:
            system.add(str(dependency))
            continue
        if str(relative) in copied:
            continue
        destination = output / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(dependency, destination)
        copied.add(str(relative))
        pending.append(dependency)
version = subprocess.check_output([str(output / 'bin' / 'rustfmt'), '--version'], text=True).strip()
(output / 'provenance.json').write_text(json.dumps({'version': version, 'libraries': sorted(copied), 'system_libraries': sorted(system)}, indent=2) + '\n')
print(version)
