#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
# SPDX-License-Identifier: Apache-2.0

"""Exercise installer downloads offline with a fake HTTPS transport and real archives."""
import hashlib
import io
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parent.parent
with tempfile.TemporaryDirectory(prefix='nysos-installer-test-') as directory:
    root = Path(directory)
    mock = root / 'mock'
    mock.mkdir()
    fixtures = root / 'fixtures'
    fixtures.mkdir()
    (mock / 'curl').write_text(f'''#!{sys.executable}
import os, pathlib, shutil, sys
args=sys.argv[1:]
assert "=https" in args
if '-w' in args:
    print('https://github.com/McCodeman/nysos/releases/tag/v1.2.3', end='')
else:
    url=next(a for a in args if a.startswith('https://'))
    source=pathlib.Path(os.environ['FIXTURES']) / url.rsplit('/',1)[1]
    if not source.exists(): sys.exit(22)
    shutil.copyfile(source, args[args.index('-o')+1])
''')
    (mock / 'uname').write_text('#!/bin/sh\ncase "$1" in -s) echo "${TEST_OS:-Linux}" ;; -m) echo "${TEST_ARCH:-x86_64}" ;; esac\n')
    for script in mock.iterdir():
        script.chmod(0o755)
    env = dict(os.environ, PATH=f'{mock}:{os.environ["PATH"]}', FIXTURES=str(fixtures), HOME=str(root / 'home'))
    archive = fixtures / 'nysos_1.2.3_linux-amd64.tar.gz'

    def payload(unsafe=False):
        with tarfile.open(archive, 'w:gz') as bundle:
            for name, data in [('bin/nysos', b'#!/bin/sh\necho test-nysos\n'), ('share/man/man1/nysos.1', b'manpage\n')]:
                info = tarfile.TarInfo(name)
                info.size = len(data)
                info.mode = 0o755 if name == 'bin/nysos' else 0o644
                bundle.addfile(info, io.BytesIO(data))
            if unsafe:
                info = tarfile.TarInfo('../escape')
                bundle.addfile(info, io.BytesIO(b''))
        (fixtures / 'SHA256SUMS').write_text(f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n')

    prefix = root / "install with ' quote"

    def install(*args, success=True, environment=None):
        result = subprocess.run(['sh', str(ROOT / 'scripts/install.sh'), '--prefix', str(prefix), *args], env=environment or env, text=True, capture_output=True)
        assert (result.returncode == 0) == success, result.stdout + result.stderr
        return result

    payload()
    for version in [(), ('--version', 'v1.2.3'), ('--version', '1.2.3')]:
        result = install(*version)
        assert subprocess.check_output([str(prefix / 'bin/nysos')], text=True).strip() == 'test-nysos'
        assert (prefix / 'share/man/man1/nysos.1').exists()
        path_line = next(line.strip() for line in result.stdout.splitlines() if line.strip().startswith('export PATH='))
        # Verify shell quoting, including apostrophes in the prefix.
        path_result = subprocess.check_output(['sh', '-c', path_line + '\nprintf "%s" "$PATH"'], env=env, text=True)
        assert path_result.startswith(str(prefix / 'bin') + ':')
    original = (prefix / 'bin/nysos').read_bytes()
    archive.write_bytes(archive.read_bytes() + b'corrupt')
    install(success=False)
    assert (prefix / 'bin/nysos').read_bytes() == original
    payload(unsafe=True)
    install(success=False)
    assert not (root / 'escape').exists()
    assert (prefix / 'bin/nysos').read_bytes() == original
    install('--version', 'v9.9.9', success=False)
    install('--version', '../../invalid', success=False)
    install('--prefix', 'relative', success=False)
    install(success=False, environment={**env, 'TEST_ARCH': 'unsupported'})
    print('PASS: latest/pinned downloads, checksums, repeat installs, quoted paths, unsupported platforms, missing assets, unsafe archives')
