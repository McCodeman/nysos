#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
# SPDX-License-Identifier: Apache-2.0

"""Exercise nysos through a private tmux client, including its prefix key table."""
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

BINARY = Path('target/debug/nysos').resolve()


def exercise(legacy=False):
    with tempfile.TemporaryDirectory(prefix='nysos-tmux-') as directory:
        root = Path(directory)
        socket = str(root / 'socket')
        done = root / 'executed'
        config = root / 'demo.toml'
        config.write_text(f'''prefix = "{'ctrl-b' if legacy else 'ctrl-g'}"
header = true
[[panes]]
name = "shell"
shell = "/bin/sh"
[[queues]]
name = "First cue"
commands = [{{pane = "shell", command = "true"}}]
[[queues]]
name = "Second cue"
commands = [{{pane = "shell", command = "touch {done}"}}]
''')
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 100, 0, 0))
        os.set_blocking(master, False)
        command = shlex.join([str(BINARY), '--config', str(config), '--terminal-keys', 'ghostty'] + (['--prefix', 'ctrl-b'] if legacy else []))
        env = dict(os.environ, TERM='xterm-256color', SSH_TTY='/dev/test-ssh', SSH_CONNECTION='test test test test')
        env.pop('TMUX', None)
        client = subprocess.Popen(['tmux', '-S', socket, '-f', '/dev/null', 'new-session', '-s', 'test', command],
                                  stdin=slave, stdout=slave, stderr=slave, env=env, start_new_session=True)
        os.close(slave)

        def tmux(*args):
            return subprocess.check_output(['tmux', '-S', socket, *args], stderr=subprocess.DEVNULL, text=True)

        def wait_for(predicate):
            deadline = time.monotonic() + 8
            while time.monotonic() < deadline:
                try:
                    while os.read(master, 65536):
                        pass
                except (BlockingIOError, OSError):
                    pass
                try:
                    if predicate():
                        return
                except subprocess.CalledProcessError:
                    pass
                time.sleep(0.03)
            raise AssertionError('tmux smoke test timed out')

        def screen():
            return tmux('capture-pane', '-p', '-t', 'test:0.0')

        def send(data):
            os.write(master, data)

        # Ctrl-B twice goes through tmux's real send-prefix binding.
        prefix = b'\x02\x02' if legacy else b'\x07'
        try:
            wait_for(lambda: 'First cue' in screen())
            tmux('set-option', '-g', 'mouse', 'on')
            # The visible cue list must receive keyboard input from launch.
            wait_for(lambda: 'CUES:' in screen())
            # Exercise terminal bytes through tmux, not just synthetic KeyEvents.
            for right, left in [(b'\x1b[1;3C', b'\x1b[1;3D'), (b'\x1bf', b'\x1bb')]:
                send(right)
                wait_for(lambda: 'CUES:' not in screen())
                send(left)
                wait_for(lambda: 'CUES:' in screen())
            # Full-editor actions are direct chords, not prefix sequences.
            for load, save, apply in [(b'\x0c', b'\x13', b'\x07'), (b'\x1bOQ', b'\x1bOR', b'\x1bOS')]:
                send(b'o')
                wait_for(lambda: 'Full demo TOML' in screen())
                send(load)
                wait_for(lambda: 'Load path' in screen())
                send(b'\r')
                wait_for(lambda: 'Full demo TOML' in screen())
                send(apply)
                wait_for(lambda: 'Full demo TOML' not in screen())
                send(b'o')
                wait_for(lambda: 'Full demo TOML' in screen())
                send(save)
                wait_for(lambda: 'Save path' in screen())
                send(b'\r')
                wait_for(lambda: 'Save path' not in screen())
                assert 'title = "nysos demo"' in config.read_text()
            send(b'\x1b[B')
            wait_for(lambda: '2/2' in screen().splitlines()[1])
            send(b'p')
            wait_for(lambda: ' Preview ' in screen())
            assert not done.exists()
            send(b'\x1b')
            wait_for(lambda: ' Preview ' not in screen())
            send(b't')
            wait_for(lambda: 'Typed without Enter' in screen())
            assert not done.exists()
            edited = root / 'edited'
            send(b'\x15' + f'touch {edited}\r'.encode())
            wait_for(edited.exists)
            assert not done.exists()
            send(prefix + b'0')
            wait_for(lambda: 'CUES:' in screen())
            send(b'e')
            wait_for(lambda: 'Single cue TOML' in screen())
            send(b'\x1b')
            wait_for(lambda: 'Single cue TOML' not in screen())
            send(b'\r')
            wait_for(done.exists)
            send(prefix + b'c')
            wait_for(lambda: ' Cues ' not in screen())
            send(prefix + b'0')
            wait_for(lambda: ' Cues ' in screen())
            # SGR click selects the first cue through tmux mouse forwarding.
            send(b'\x1b[<0;3;6M\x1b[<0;3;6m')
            wait_for(lambda: '1/2' in screen().splitlines()[1])
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 90, 0, 0))
            client.send_signal(signal.SIGWINCH)
            wait_for(lambda: tmux('display-message', '-p', '-t', 'test:0.0', '#{pane_width}x#{pane_height}').strip() == '90x23')
            send(prefix + b'q')
            wait_for(lambda: client.poll() is not None)
            assert client.returncode == 0
        finally:
            subprocess.run(['tmux', '-S', socket, 'kill-server'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            if client.poll() is None:
                client.kill()
                client.wait(timeout=5)
            os.close(master)


for legacy in (False, True):
    exercise(legacy)
    print('PASS: tmux client, SSH environment, full-editor Ctrl/F-key load/save/apply, cues, Alt/Option focus, mouse, resize, quit; prefix=' + ('ctrl-b' if legacy else 'ctrl-g'))
