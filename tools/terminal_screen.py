"""Test-only VT screen view with correct replacement of wide-character cells.

pyte retains the empty trailing cell when a two-cell glyph is overwritten by a
one-cell glyph, and its display property then indexes that empty string. Real
terminals clear the overwritten glyph. Preserve the parser's data/styles while
rendering these replaced cells as spaces; intact wide glyphs keep both cells.
"""

import pyte
from wcwidth import wcwidth


class Screen(pyte.Screen):
    @property
    def display(self):
        rows = []
        for y in range(self.lines):
            line, output, x = self.buffer[y], [], 0
            while x < self.columns:
                value = line[x].data
                if not value:
                    output.append(" ")
                elif wcwidth(value[0]) == 2:
                    if x + 1 < self.columns and line[x + 1].data == "":
                        output.append(value)
                        x += 2
                        continue
                    output.append(" ")
                else:
                    output.append(value)
                x += 1
            rows.append("".join(output))
        return rows
