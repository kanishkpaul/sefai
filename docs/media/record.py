"""Record the README's terminal images from the compiled TUI in a real PTY, in colour.

  python3 docs/media/record.py chat BINARY GGUF   # SmolLM2-135M Q4_K_M on Metal answers two turns

BINARY is a release build with `metal`; GGUF is the pinned fixture in tests/fixtures/gguf.json.
Each run uses a fresh temporary workspace, writes docs/media/<scene>.cast as asciicast v2, and
marks the frames render.py draws with marker events.
"""
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time

MEDIA = Path(__file__).resolve().parent
COLUMNS, ROWS = 92, 30
ESCAPE = re.compile(r'\x1b\[[0-9;?]*[@-~]')
SCENES = {
    'chat': {
        'profile': 'id: small\nformat: gguf\npath: {model}\nrunner:\n  backend: metal\n  device: 0\n  context: 4096\n',
        'args': ['--profile', 'small', 'chat'],
        'steps': [
            ('type', 'In one sentence, what is a river delta?'), ('wait', 'ctx '), ('settle', None),
            ('type', 'Now as a haiku.'), ('wait', 'ctx '), ('settle', None), ('mark', 'chat'),
        ],
    },
}


def main():
    if len(sys.argv) != 4 or sys.argv[1] not in SCENES:
        raise SystemExit('usage: record.py chat BINARY MODEL')
    name, binary, model = sys.argv[1], Path(sys.argv[2]).resolve(), Path(sys.argv[3]).resolve()
    scene = SCENES[name]
    with tempfile.TemporaryDirectory(prefix='sefai-media-') as workspace:
        root = Path(workspace) / 'workspace'
        subprocess.run([str(binary), '--workspace', str(root), 'workspace', 'init'], check=True,
                       stdout=subprocess.DEVNULL)
        profile = scene['profile'].format(model=json.dumps(str(model)))
        (root / 'models' / (scene['args'][1] + '.md')).write_text(f'---\nschema: 1\n{profile}---\n')
        events = record(binary, root, scene)
    header = {'version': 2, 'width': COLUMNS, 'height': ROWS, 'env': {'TERM': 'xterm-256color'}}
    lines = [json.dumps(header)] + [json.dumps(event, ensure_ascii=False) for event in events]
    (MEDIA / (name + '.cast')).write_text('\n'.join(lines) + '\n')
    print(f'{name}.cast: {len(events)} events')
    if ['m', 'timeout'] in [event[1:] for event in events]:
        raise SystemExit('the recording timed out; see the cast')


def record(binary, workspace, scene):
    leader, follower = pty.openpty()
    fcntl.ioctl(follower, termios.TIOCSWINSZ, struct.pack('HHHH', ROWS, COLUMNS, 0, 0))
    environment = {key: value for key, value in os.environ.items() if key != 'NO_COLOR'}
    environment.update(TERM='xterm-256color', COLORTERM='truecolor', SEFAI_THEME='dark')
    child = subprocess.Popen([str(binary), '--workspace', str(workspace), *scene['args']],
                             stdin=follower, stdout=follower, stderr=follower, env=environment,
                             start_new_session=True)
    os.close(follower)
    start, events = time.monotonic(), []

    def pump(seconds):
        deadline = time.monotonic() + seconds
        received = False
        while (remaining := deadline - time.monotonic()) > 0:
            ready, _, _ = select.select([leader], [], [], remaining)
            if not ready:
                break
            try:
                chunk = os.read(leader, 65536).decode('utf-8', 'replace')
            except OSError:
                break
            if not chunk:
                break
            events.append([round(time.monotonic() - start, 3), 'o', chunk])
            received = True
        return received

    def send(text):
        os.write(leader, text.encode())
        events.append([round(time.monotonic() - start, 3), 'i', text])

    pump(2.0)
    for action, value in scene['steps']:
        if action == 'type':
            for character in value:
                send(character)
                pump(0.02)
            send('\r')
        elif action == 'wait':
            # Only output after this step counts, with escapes removed so styled runs still match.
            begin, deadline = len(events), time.monotonic() + 180
            seen = ''
            while value not in seen and time.monotonic() < deadline:
                pump(0.5)
                seen = ESCAPE.sub('', ''.join(event[2] for event in events[begin:] if event[1] == 'o'))
            if value not in seen:
                events.append([round(time.monotonic() - start, 3), 'm', 'timeout'])
                print(f'timed out waiting for {value!r}', file=sys.stderr)
                break
        elif action == 'settle':
            pump(1.0)
            deadline = time.monotonic() + 180
            while pump(2.5) and time.monotonic() < deadline:
                pass
        elif action == 'mark':
            events.append([round(time.monotonic() - start, 3), 'm', value])
    send('\x04')
    pump(2.0)
    child.wait(timeout=10)
    os.close(leader)
    return events


main()
