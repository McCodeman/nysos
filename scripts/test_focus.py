#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
# SPDX-License-Identifier: Apache-2.0

"""Verify raw terminal encodings change the shell receiving subsequent input."""
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time

def exercise(profile, program, term, ghostty):
    with tempfile.TemporaryDirectory(prefix='nysos-focus-') as directory:
        root = Path(directory)
        config = root / 'demo.toml'
        config.write_text('''cue_list = false
    queues = []
    [[panes]]
    name = "first"
    shell = "/bin/sh"
    args = ["-c", "export NYSOS_FOCUS_TEST=first; exec /bin/sh"]
    [[panes]]
    name = "second"
    shell = "/bin/sh"
    args = ["-c", "export NYSOS_FOCUS_TEST=second; exec /bin/sh"]
    ''')
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 100, 0, 0))
        child = subprocess.Popen([str(Path('target/debug/nysos').resolve()), '--config', str(config)] + (['--terminal-keys', profile] if profile else []),
                                 stdin=slave, stdout=slave, stderr=slave, env=dict(os.environ, TERM=term, TERM_PROGRAM=program))
        os.close(slave)
        os.set_blocking(master, False)
        captured = bytearray()

        def wait_for(predicate):
            deadline = time.monotonic() + 8
            while time.monotonic() < deadline:
                if select.select([master], [], [], 0.03)[0]:
                    try:
                        captured.extend(os.read(master, 65536))
                    except OSError:
                        pass
                if predicate():
                    return
            raise AssertionError('focus test timed out')

        try:
            wait_for(lambda: b'first' in captured and b'second' in captured)
            for index, (sequence, expected) in enumerate([
                (b'\x1b[1;3C', 'second'),  # standard Alt-Right
                (b'\x1b[1;3D', 'first'),   # standard Alt-Left
                (b'\x1bf', 'second' if ghostty else 'first'),      # Ghostty Option-Right
                (b'\x1bb', 'first'),       # Ghostty Option-Left
            ]):
                result = root / f'focus-{index}'
                os.write(master, sequence)
                os.write(master, f'printf "$NYSOS_FOCUS_TEST" > {result}\r'.encode())
                wait_for(lambda: result.exists() and result.read_text() == expected)
                print(f'PASS: profile={profile or "auto"}, terminal={program or term}: {sequence!r} focuses {expected} shell')
            os.write(master, b'\x07q')
            wait_for(lambda: child.poll() is not None)
            assert child.returncode == 0
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=5)
            os.close(master)


exercise(None, "ghostty", "xterm-256color", True)
exercise(None, "", "xterm-ghostty", True)
exercise(None, "Apple_Terminal", "xterm-256color", False)
exercise("standard", "ghostty", "xterm-ghostty", False)
exercise("ghostty", "tmux", "tmux-256color", True)
