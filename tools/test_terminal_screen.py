"""Synthetic VT cell replacement regressions; no window or user terminal involved."""

import unittest
import pyte
from terminal_screen import Screen


class CellReplacementTests(unittest.TestCase):
    def test_replacing_leading_cell_clears_orphaned_wide_stub(self):
        screen = Screen(8, 2)
        pyte.Stream(screen).feed("中文\x1b[1;1Hx")
        self.assertEqual(screen.display[0], "x 文    ")

    def test_replacing_trailing_cell_clears_leading_half(self):
        screen = Screen(8, 2)
        pyte.Stream(screen).feed("中\x1b[1;2Hy")
        self.assertEqual(screen.display[0], " y      ")

    def test_intact_wide_text_and_attributes_are_preserved(self):
        screen = Screen(8, 2)
        pyte.Stream(screen).feed("\x1b[31;1m中文A")
        self.assertEqual(screen.display[0], "中文A   ")
        self.assertTrue(screen.buffer[0][0].bold)
        self.assertEqual(screen.buffer[0][0].fg, "red")


if __name__ == "__main__":
    unittest.main(verbosity=2)
