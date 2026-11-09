"""Replay a recorded asciicast up to one marker through a small terminal model and write framed HTML.

  python3 docs/media/render.py chat chat   # docs/media/chat.cast at marker "chat" -> chat.html

The model covers what the Ratatui/Crossterm frontend emits: printable text with East Asian wide
cells, CR/LF, cursor positioning, truecolour foreground and background SGR, bold, reset and erase.
Private modes are ignored. Colours are the bytes the TUI wrote, from the design kit's Candle
palette; cells the TUI left uncoloured use the kit's Candle dark surface and text. Nothing is
retouched. The README image was captured from the HTML in headless Chrome at device scale 2, trimmed to the window.
"""
import html
import json
import re
import sys
import unicodedata
from pathlib import Path

MEDIA = Path(__file__).resolve().parent
BACKGROUND, DEFAULT_TEXT = '#120E0D', '#FBF1EA'
CONTROL = re.compile(r'\x1b(?:\[([0-9;?]*)([@-~])|\][^\x07\x1b]*(?:\x07|\x1b\\)|[78=>]|\(.)')


def main():
    if len(sys.argv) != 3:
        raise SystemExit('usage: render.py CAST MARKER')
    name, marker = sys.argv[1], sys.argv[2]
    lines = (MEDIA / (name + '.cast')).read_text().splitlines()
    header, events = json.loads(lines[0]), [json.loads(line) for line in lines[1:]]
    output = []
    for _, kind, data in events:
        if kind == 'm' and data == marker:
            break
        if kind == 'o':
            output.append(data)
    else:
        raise SystemExit(f'marker {marker!r} not found')
    screen = Screen(header['width'], header['height'])
    screen.feed(''.join(output))
    body = '\n'.join(row_html(row) for row in screen.rows)
    page = TEMPLATE.format(background=BACKGROUND, width=header['width'], body=body)
    (MEDIA / (marker + '.html')).write_text(page)
    print(f'{marker}.html: {header["width"]}x{header["height"]}')


class Screen:
    def __init__(self, width, height):
        self.width, self.height = width, height
        self.clear()
        self.row = self.col = 0
        self.style = (None, None, False)

    def clear(self):
        self.rows = [[(' ', None, None, False) for _ in range(self.width)] for _ in range(self.height)]

    def feed(self, text):
        position = 0
        for match in CONTROL.finditer(text):
            self.text(text[position:match.start()])
            position = match.end()
            if match.group(2):
                self.csi(match.group(1), match.group(2))
        self.text(text[position:])

    def text(self, text):
        for character in text:
            if character == '\n':
                self.row = min(self.row + 1, self.height - 1)
            elif character == '\r':
                self.col = 0
            elif character == '\b':
                self.col = max(self.col - 1, 0)
            elif character >= ' ' and not unicodedata.combining(character):
                wide = unicodedata.east_asian_width(character) in 'WF'
                if self.col + (2 if wide else 1) > self.width:
                    self.row, self.col = min(self.row + 1, self.height - 1), 0
                self.rows[self.row][self.col] = (character, *self.style)
                if wide:
                    self.rows[self.row][self.col + 1] = ('', *self.style)
                self.col += 2 if wide else 1
            elif character >= ' ' and self.col > 0:
                previous = self.rows[self.row][self.col - 1]
                self.rows[self.row][self.col - 1] = (previous[0] + character, *previous[1:])

    def csi(self, params, command):
        if params.startswith('?'):
            if command == 'h' and params in ('?1049', '?47', '?1047'):
                self.clear()
            return
        values = [int(value) if value else 0 for value in params.split(';')] if params else []
        first = values[0] if values else 0
        if command in 'Hf':
            self.row = min(max((values[0] if values else 1) - 1, 0), self.height - 1)
            self.col = min(max((values[1] if len(values) > 1 else 1) - 1, 0), self.width - 1)
        elif command == 'G':
            self.col = min(max(first - 1, 0), self.width - 1)
        elif command in 'ABCD':
            step = max(first, 1)
            if command == 'A':
                self.row = max(self.row - step, 0)
            elif command == 'B':
                self.row = min(self.row + step, self.height - 1)
            elif command == 'C':
                self.col = min(self.col + step, self.width - 1)
            else:
                self.col = max(self.col - step, 0)
        elif command == 'J':
            blank = (' ', None, None, False)
            if first == 2 or first == 3:
                self.clear()
            elif first == 0:
                self.rows[self.row][self.col:] = [blank] * (self.width - self.col)
                for row in self.rows[self.row + 1:]:
                    row[:] = [blank] * self.width
        elif command == 'K':
            blank = (' ', None, None, False)
            if first == 0:
                self.rows[self.row][self.col:] = [blank] * (self.width - self.col)
            elif first == 2:
                self.rows[self.row][:] = [blank] * self.width
        elif command == 'm':
            self.sgr(values or [0])

    def sgr(self, values):
        foreground, background, bold = self.style
        index = 0
        while index < len(values):
            value = values[index]
            if value == 0:
                foreground, background, bold = None, None, False
            elif value == 1:
                bold = True
            elif value == 22:
                bold = False
            elif value == 39:
                foreground = None
            elif value == 49:
                background = None
            elif value in (38, 48) and values[index + 1:index + 2] == [2]:
                colour = '#%02X%02X%02X' % tuple(values[index + 2:index + 5])
                foreground, background = (colour, background) if value == 38 else (foreground, colour)
                index += 4
            elif value in (38, 48) and values[index + 1:index + 2] == [5]:
                index += 2
            index += 1
        self.style = (foreground, background, bold)


def row_html(row):
    out, current, run = [], None, ''
    for character, foreground, background, bold in row + [('', 'end', None, False)]:
        style = (foreground, background, bold)
        if style != current:
            if run and current is not None:
                css = f'color:{current[0] or DEFAULT_TEXT}'
                if current[1]:
                    css += f';background:{current[1]}'
                if current[2]:
                    css += ';font-weight:700'
                out.append(f'<span style="{css}">{html.escape(run)}</span>')
            current, run = style, ''
        run += character
    return ''.join(out)


TEMPLATE = '''<!doctype html><meta charset="utf-8">
<style>
  html, body {{ margin: 0; background: transparent; }}
  .window {{ display: inline-block; margin: 24px; border-radius: 12px; overflow: hidden;
    background: {background}; box-shadow: 0 0 0 1px #3A2B26, 0 18px 50px rgba(0,0,0,.45); }}
  .bar {{ height: 34px; display: flex; align-items: center; gap: 8px; padding: 0 14px;
    background: #1C1715; color: #CDC2BC; font: 600 13px -apple-system, system-ui, sans-serif; }}
  .dot {{ width: 12px; height: 12px; border-radius: 50%; }}
  .title {{ flex: 1; text-align: center; margin-right: 52px; }}
  pre {{ margin: 0; padding: 14px 18px 18px; font: 13.5px/1.17 Menlo, "SF Mono", monospace;
    width: {width}ch; white-space: pre; }}
</style>
<div class="window"><div class="bar"><span class="dot" style="background:#FF5F57"></span>
<span class="dot" style="background:#FEBC2E"></span><span class="dot" style="background:#28C840"></span>
<span class="title">sefai</span></div><pre>{body}</pre></div>
'''

main()
