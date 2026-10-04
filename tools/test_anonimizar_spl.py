"""Tests del anonimizador con un .spl sintético (sin datos reales).

Ejecutar: python3 -m unittest discover -s tools -p "test_*.py"
"""
import os
import struct
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import anonimizar_spl as anon  # noqa: E402
from anonimizar_spl import ref  # noqa: E402


def text(tag, s):
    raw = s.encode("latin-1")
    return bytes([tag]) + struct.pack("<H", len(raw)) + raw


def u8(tag, v):
    return bytes([tag, v])


def u16(tag, v):
    return bytes([tag]) + struct.pack("<H", v)


def u32(tag, v):
    return bytes([tag]) + struct.pack("<I", v)


def punches(items):
    out = bytes([0x97]) + struct.pack("<H", len(items))
    for code, cs in items:
        out += struct.pack("<H", code) + cs.to_bytes(3, "little")
    return out


def runner(rid, given, family, club, club_id, bib, card, items, status, place, birth):
    return (u32(0x80, rid) + u32(0x81, bib) + u32(0x84, card) + text(0x87, given)
            + text(0x88, family) + u32(0x89, club_id) + text(0x8C, club)
            + punches(items) + u8(0x98, status) + u16(0x99, place) + u8(0x9A, 2)
            + bytes([0x9B]) + struct.pack("<d", birth))


def synthetic_spl():
    header = bytearray(b"spl4" + bytes(0x60 - 4))
    struct.pack_into("<d", header, 0x4C, 46298.0)  # 2026-10-03
    header += text(0x18, "Club ORCA") + bytes(4)
    legs = b"".join(struct.pack("<HHI", a, b, 0) for a, b in [(32736, 31), (31, 45), (45, 32752)])
    cls = u32(0x40, 1) + text(0x43, "F21A") + bytes([0x47]) + struct.pack("<I", len(legs)) + legs
    nine = 9 * 3600 * 100
    r1 = runner(10, "José", "García López", "ORCA", 77, 101, 2000123,
                [(32736, nine), (31, nine + 9000), (45, nine + 15000), (32752, nine + 18000)],
                0, 1, 30000.5)
    r2 = runner(11, "Ana", "Pérez", "Montaña Club", 88, 0, 2000456,
                [(32736, nine), (31, ref.MISSING), (45, nine + 21000), (32752, nine + 24000)],
                6, 0, 31000.0)
    return bytes(header) + cls + r1 + r2 + bytes([0x9A])  # último registro truncado


class AnonymizeTest(unittest.TestCase):
    def setUp(self):
        self.original = synthetic_spl()
        self.anon = anon.anonymize(self.original)
        self.parsed = ref.parse(self.anon, keep_birthdate=True)
        self.runners = self.parsed["classes"][0]["runners"]

    def test_keeps_size_times_and_results(self):
        self.assertEqual(len(self.anon), len(self.original))
        before = ref.parse(self.original)["classes"][0]
        after = self.parsed["classes"][0]
        self.assertEqual(after["course"], [31, 45, 32752])
        for a, b in zip(before["runners"], after["runners"]):
            for k in ("id", "punches", "status", "place", "sex"):
                self.assertEqual(a.get(k), b.get(k), k)
        self.assertIsNone(self.runners[1]["punches"][1]["time_of_day_s"])

    def test_replaces_personal_data(self):
        r1, r2 = self.runners
        self.assertEqual((r1["given"], r1["family"]), ("N1__", "Apellido1___"))
        self.assertEqual((r2["given"], r2["family"]), ("N2_", "A2___"))
        self.assertEqual((r1["club"], r2["club"]), ("CA__", "Club B______"))
        self.assertEqual((r1["club_id"], r2["club_id"]), (1, 2))
        self.assertEqual((r1["bib"], r1["si_card"]), (1, 1))
        self.assertEqual((r2["bib"], r2["si_card"]), (0, 2))  # dorsal 0 se queda en 0
        self.assertEqual({r["birthdate"] for r in self.runners}, {"1899-12-30"})

    def test_no_original_text_left(self):
        for s in ("José", "García", "López", "ORCA", "Ana", "Pérez", "Montaña"):
            self.assertNotIn(s.encode("latin-1"), self.anon, s)
        anon.check(self.original, self.anon)  # no lanza

    def test_check_rejects_unanonymized_file(self):
        with self.assertRaises(AssertionError):
            anon.check(self.original, self.original)


class PseudonymTest(unittest.TestCase):
    def test_always_fills_the_exact_length(self):
        for n in range(1, 30):
            self.assertEqual(len(anon.pseudonym("Nombre", 275, n)), n)
            self.assertEqual(len(anon.club_label(30, n)), n)

    def test_short_fields_stay_distinct(self):
        self.assertEqual(anon.pseudonym("Nombre", 275, 2), "7N")
        self.assertEqual(anon.club_label(26, 4), "CAA_")


if __name__ == "__main__":
    unittest.main()
