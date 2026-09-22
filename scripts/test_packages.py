#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
# SPDX-License-Identifier: Apache-2.0

"""Install release packages in disposable Linux containers and smoke-test real PTYs."""
import argparse
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import tarfile
import tempfile
import termios
import time
import tomllib

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--platform', required=True, choices=['linux-amd64', 'linux-arm64', 'darwin-amd64', 'darwin-arm64'])
args = parser.parse_args()
version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
name = f'nysos_{version}_{args.platform}'
dist = ROOT / 'dist'
with tempfile.TemporaryDirectory(prefix='nysos-package-test-') as directory:
    root = Path(directory)
    with tarfile.open(dist / f'{name}.tar.gz') as archive:
        archive.extractall(root, filter='data')
    binary = root / 'bin/nysos'
    for path in ['bin/nysos', 'share/man/man1/nysos.1', 'share/doc/nysos/LICENSE', 'share/doc/nysos/nysos.spdx.json', 'share/bash-completion/completions/nysos', 'share/zsh/site-functions/_nysos']:
        assert (root / path).is_file(), path
    if args.platform.startswith('linux'):
        docker_platform = args.platform.replace('-', '/')
        base = ['docker', 'run', '--rm', '--platform', docker_platform]
        for image, command in [
            ('debian:bookworm-slim', f'apt-get install -y /packages/{name}.deb && nysos --demo --check'),
            ('ubuntu:24.04', f'apt-get install -y /packages/{name}.deb && nysos --demo --check'),
            ('fedora:42', f'dnf -y --disablerepo="*" install /packages/{name}.rpm && nysos --demo --check'),
        ]:
            output = subprocess.check_output([*base, '-v', f'{dist}:/packages:ro', image, 'sh', '-ec', command], text=True)
            assert 'Valid: 3 panes, 6 queue items' in output, output
            print(f'PASS: {args.platform} installed with package manager in {image}')
        command = [*base, '-it', '-e', 'TERM=xterm-256color', '-v', f'{binary}:/usr/local/bin/nysos:ro', 'alpine:3.23', 'nysos', '--demo']
    else:
        assert 'Valid: 3 panes, 6 queue items' in subprocess.check_output([str(binary), '--demo', '--check'], text=True)
        command = [str(binary), '--demo']
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 36, 140, 0, 0))
    process = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave, env={**os.environ, 'TERM': 'xterm-256color'})
    os.close(slave)
    output = bytearray()

    def wait_for(predicate):
        deadline = time.monotonic() + 45
        while time.monotonic() < deadline:
            if select.select([master], [], [], 0.05)[0]:
                try:
                    output.extend(os.read(master, 65536))
                except OSError:
                    pass
            if predicate():
                return
        raise AssertionError(bytes(output[-4000:]))

    try:
        wait_for(lambda: b'Service' in output and b'Notes' in output)
        os.write(master, b'\x070\r')
        wait_for(lambda: b'Service:' in output)
        os.write(master, b'\x07q')
        wait_for(lambda: process.poll() is not None)
        assert process.returncode == 0
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
    print(f'PASS: {args.platform} archive starts PTY shells, executes a cue, and exits cleanly')
