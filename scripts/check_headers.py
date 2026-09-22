#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
# SPDX-License-Identifier: Apache-2.0

"""Check first-party source headers, including exported pre-commit snapshots."""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXCLUDED = {'.git', 'target', '.direnv', '.venv', 'site', '.cache', '__pycache__'}
EXTENSIONS = {'.rs', '.py', '.sh', '.nix', '.toml', '.yml', '.yaml', '.md', '.roff', '.1'}
SPECIAL = {'Makefile', '.envrc', '.gitignore', 'pre-commit'}
COPYRIGHT = 'Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)'
errors = []
checked = 0
for path in ROOT.rglob('*'):
    relative = path.relative_to(ROOT)
    if any(part in EXCLUDED for part in relative.parts) or path.is_symlink() or not path.is_file():
        continue
    if path.suffix in {'.json', '.lock'}:
        sidecar = path.with_name(path.name + '.license')
        expected_license = 'CC0-1.0' if path.name.endswith('.spdx.json') else 'Apache-2.0'
        header = sidecar.read_text() if sidecar.exists() else ''
        if COPYRIGHT not in header or f'SPDX-License-Identifier: {expected_license}' not in header:
            errors.append(str(relative) + ' (sidecar)')
        checked += 1
        continue
    if path.suffix not in EXTENSIONS and path.name not in SPECIAL:
        continue
    header = path.read_text()[:1024]
    if COPYRIGHT not in header or 'SPDX-License-Identifier: Apache-2.0' not in header:
        errors.append(str(relative))
    checked += 1
if errors:
    raise SystemExit('Missing copyright/SPDX headers: ' + ', '.join(errors))
print(f'Copyright/SPDX headers: {checked} files checked')
