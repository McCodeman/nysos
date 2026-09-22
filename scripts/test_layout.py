#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
# SPDX-License-Identifier: Apache-2.0

"""Real client tests: nested splits, PTY sizes, SGR dragging, and keyboard fallback."""
import fcntl
import os
from pathlib import Path
import pty
import shlex
import signal
import struct
import subprocess
import tempfile
import termios
import time
import tomllib

BINARY = Path(os.environ.get('NYSOS_BINARY', 'target/debug/nysos')).resolve()


def exercise(no_mouse):
    with tempfile.TemporaryDirectory(prefix='nysos-layout-') as directory:
        root = Path(directory)
        socket = str(root / 'socket')
        config = root / 'demo.toml'
        config.write_text(Path('examples/nested-layout.toml').read_text())
        size_file = root / 'size'
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 146, 0, 0))
        os.set_blocking(master, False)
        command = shlex.join([str(BINARY), '--config', str(config)] + (['--no-mouse'] if no_mouse else []))
        env = dict(os.environ, TERM='xterm-256color', SSH_TTY='/dev/test-ssh', SSH_CONNECTION='test test test test')
        env.pop('TMUX', None)
        client = subprocess.Popen(['tmux', '-S', socket, '-f', '/dev/null', 'new-session', '-s', 'layout', command], stdin=slave, stdout=slave, stderr=slave, env=env, start_new_session=True)
        os.close(slave)

        def tmux(*args):
            return subprocess.check_output(['tmux', '-S', socket, *args], text=True, stderr=subprocess.DEVNULL)

        def screen():
            return tmux('capture-pane', '-p', '-t', 'layout:0.0')

        def wait_for(predicate):
            end = time.monotonic() + 10
            while time.monotonic() < end:
                try:
                    while os.read(master, 65536):
                        pass
                except (BlockingIOError, OSError):
                    pass
                try:
                    if predicate():
                        return
                except (subprocess.CalledProcessError, IndexError, ValueError, FileNotFoundError):
                    pass
                time.sleep(0.03)
            raise AssertionError('layout test timed out:\n' + screen())

        def send(data):
            os.write(master, data)

        def shell_size():
            size_file.unlink(missing_ok=True)
            send(b'\x071' + f'stty size > {shlex.quote(str(size_file))}\r'.encode())
            wait_for(lambda: size_file.exists() and len(size_file.read_text().split()) == 2)
            return tuple(map(int, size_file.read_text().split()))

        def save():
            send(b'\x07o')
            wait_for(lambda: 'Full demo TOML' in screen())
            send(b'\x13')
            wait_for(lambda: 'Save path' in screen())
            send(b'\r')
            wait_for(lambda: 'Save path' not in screen())
            return tomllib.loads(config.read_text())['layout']

        def zoom(rows, cols):
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
            client.send_signal(signal.SIGWINCH)
            wait_for(lambda: tmux('display-message', '-p', '-t', 'layout:0.0', '#{pane_width}x#{pane_height}').strip() == f'{cols}x{rows - 1}')
            # tmux resizes its screen before the application's SIGWINCH has
            # propagated to child PTYs. Wait for nysos to redraw its bottom-right
            # pane corner at the new dimensions before querying a shell's size.
            wait_for(lambda: screen().splitlines()[rows - 4][cols - 1] == '┘')

        try:
            wait_for(lambda: 'Presenter' in screen() and 'Notes' in screen())
            wait_for(lambda: screen().splitlines()[36][145] == '┘')
            tmux('set-option', '-g', 'mouse', 'off' if no_mouse else 'on')
            before_layout = tomllib.loads(config.read_text())['layout']
            before = shell_size()
            # Font size changes are delivered as PTY cell-size changes like these.
            zoom(30, 106)
            smaller = shell_size()
            assert smaller[0] < before[0] and smaller[1] < before[1], (before, smaller)
            zoom(40, 146)
            assert shell_size() == before
            # Save normalizes omitted weights, so compare against a normalized baseline.
            normalized = save()
            assert normalized['direction'] == before_layout['direction']
            send(b'\x071\x07>\x07+')
            after = shell_size()
            assert after[0] > before[0] and after[1] > before[1], (before, after)
            if not no_mouse:
                lines = screen().splitlines()
                border = lines[4].index(' 4 · Logs') - 1
                # Press on the outer split, then drag right and release through tmux.
                send(f'\x1b[<0;{border + 1};7M\x1b[<32;{border + 7};7M\x1b[<0;{border + 7};7m'.encode())
                wait_for(lambda: screen().splitlines()[4].index(' 4 · Logs') - 1 > border)
                assert shell_size()[1] > after[1]
            saved = save()
            assert saved != normalized
            # Nested weights survive applying/saving, and resize remains reversible.
            stable = shell_size()
            zoom(28, 96)
            assert shell_size()[1] < stable[1]
            zoom(40, 146)
            assert shell_size() == stable
            send(b'\x07q')
            wait_for(lambda: client.poll() is not None)
            assert client.returncode == 0
        finally:
            subprocess.run(['tmux', '-S', socket, 'kill-server'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            if client.poll() is None:
                client.kill()
                client.wait(timeout=5)
            os.close(master)


for no_mouse in (False, True):
    exercise(no_mouse)
    print('PASS: nested layout, font-size cell changes, child PTY sizes, save/apply, ' + ('keyboard-only fallback' if no_mouse else 'SGR drag through tmux'))
