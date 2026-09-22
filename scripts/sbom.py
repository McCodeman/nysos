#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
# SPDX-License-Identifier: Apache-2.0

"""Generate a Cargo SPDX inventory; validate it and check lockfile coverage."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import tomllib

from spdx_tools.spdx.parser.parse_anything import parse_file
from spdx_tools.spdx.validation.document_validator import validate_full_spdx_document

ROOT = Path(__file__).resolve().parent.parent
os.chdir(ROOT)
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--check', action='store_true')
args = parser.parse_args()
path = ROOT / 'sbom/nysos.spdx.json'
lock = Path('Cargo.lock').read_bytes()
manifest = tomllib.loads(Path('Cargo.toml').read_text())['package']
metadata = json.loads(subprocess.check_output(
    ['cargo', 'metadata', '--locked', '--format-version', '1'], text=True))
nodes = {node['id']: node for node in metadata['resolve']['nodes']}
pending = [metadata['resolve']['root']]
reachable = set()
while pending:
    node = pending.pop()
    if node in reachable:
        continue
    reachable.add(node)
    pending.extend(dep['pkg'] for dep in nodes[node]['deps']
                   if any(kind['kind'] != 'dev' for kind in dep['dep_kinds']))
expected = {(p['name'], p['version']) for p in metadata['packages'] if p['id'] in reachable}
if not args.check:
    # Fetch exactly the locked graph, then prohibit network resolution by cargo-sbom.
    subprocess.run(['cargo', 'fetch', '--locked'], check=True)
    document = json.loads(subprocess.check_output(
        ['cargo', 'sbom', '--output-format', 'spdx_json_2_3'],
        env={**os.environ, 'CARGO_NET_OFFLINE': 'true'}, text=True))
    if Path('Cargo.lock').read_bytes() != lock:
        raise SystemExit('cargo-sbom changed Cargo.lock; review before proceeding')
    # cargo-sbom 0.10 can omit one version of a repeated crate name. Restore
    # the exact graph from Cargo rather than silently shipping an incomplete SBOM.
    packages = {p['id']: p for p in metadata['packages'] if p['id'] in reachable}
    existing_ids = {(p['name'], p['versionInfo']): p['SPDXID'] for p in document['packages']}
    ids = {key: existing_ids.get((p['name'], p['version']),
           re.sub(r'[^A-Za-z0-9.-]', '-', f"SPDXRef-Package-{p['name']}-{p['version']}"))
           for key, p in packages.items()}
    present = {p['SPDXID'] for p in document['packages']}
    for key, package in packages.items():
        if ids[key] not in present:
            document['packages'].append({
                'SPDXID': ids[key], 'name': package['name'],
                'versionInfo': package['version'], 'downloadLocation': 'NOASSERTION',
                'licenseDeclared': (package['license'] or 'NOASSERTION').replace('/', ' OR '),
            })
    edges = {(ids[key], ids[dep['pkg']]) for key in reachable
             for dep in nodes[key]['deps']
             if any(kind['kind'] != 'dev' for kind in dep['dep_kinds'])}
    document['relationships'] = [
        {'spdxElementId': source, 'relationshipType': 'DEPENDS_ON', 'relatedSpdxElement': target}
        for source, target in edges
    ] + [{'spdxElementId': 'SPDXRef-DOCUMENT', 'relationshipType': 'DESCRIBES',
          'relatedSpdxElement': ids[metadata['resolve']['root']]}]
    checksums = {(p['name'], p['version']): p.get('checksum')
                 for p in tomllib.loads(lock.decode())['package']}
    for package in document['packages']:
        package['filesAnalyzed'] = False
        # Cargo metadata declares licenses; it does not establish a legal conclusion.
        package['licenseConcluded'] = 'NOASSERTION'
        package['copyrightText'] = 'NOASSERTION'
        checksum = checksums.get((package['name'], package['versionInfo']))
        if checksum:
            package['checksums'] = [{'algorithm': 'SHA256', 'checksumValue': checksum}]
            package['downloadLocation'] = f"https://crates.io/api/v1/crates/{package['name']}/{package['versionInfo']}/download"
        if package['name'] == manifest['name']:
            package['copyrightText'] = 'Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)'
    document['creationInfo']['comment'] = (
        'Cargo source dependency inventory across platforms, normal and build dependencies. '
        'Excludes development-only crates, Nix, Python documentation tools, system libraries, and final binary analysis. '
        'Cargo.lock SHA256: ' + hashlib.sha256(lock).hexdigest())
    document['packages'].sort(key=lambda item: item['SPDXID'])
    document['relationships'].sort(key=lambda item: (item['spdxElementId'], item['relationshipType'], item['relatedSpdxElement']))
    path.parent.mkdir(exist_ok=True)
    temporary = path.with_name('nysos.tmp.spdx.json')
    temporary.write_text(json.dumps(document, indent=2) + '\n')
    errors = validate_full_spdx_document(parse_file(str(temporary)))
    if errors:
        temporary.unlink()
        raise SystemExit('\n'.join(str(error) for error in errors))
    temporary.replace(path)

document = json.loads(path.read_text())
errors = validate_full_spdx_document(parse_file(str(path)))
if errors:
    raise SystemExit('\n'.join(str(error) for error in errors))

actual = {(p['name'], p['versionInfo']) for p in document['packages']}
if actual != expected:
    raise SystemExit(f'SBOM package coverage differs from Cargo.lock: missing={expected-actual}, extra={actual-expected}')
root = next(p for p in document['packages'] if p['name'] == manifest['name'])
if root['versionInfo'] != manifest['version'] or root['licenseDeclared'] != manifest['license']:
    raise SystemExit('SBOM project metadata is stale; run make sbom')
if hashlib.sha256(lock).hexdigest() not in document['creationInfo']['comment']:
    raise SystemExit('SBOM Cargo.lock digest is stale; run make sbom')
print(f'SPDX 2.3 validated: {len(actual)} packages, {len(document["relationships"])} relationships')
