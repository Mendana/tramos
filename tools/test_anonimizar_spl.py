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
from test_winsplits_spl import header, klass, runner_record, text, u8, u16, u32  # noqa: E402


def punches(items):
    out = bytes([0x97]) + struct.pack("<H", len(items))
    for code, cs in items:
        out += struct.pack("<H", code) + cs.to_bytes(3, "little")
    return out


def runner(given, family, club, club_id, bib, card, items, status, place, birth, extra=b""):
    return runner_record(u32(0x81, bib) + u32(0x84, card) + text(0x87, given)
                         + text(0x88, family) + u32(0x89, club_id) + text(0x8C, club)
                         + punches(items) + u8(0x98, status) + u16(0x99, place) + u8(0x9A, 2)
                         + bytes([0x9B]) + struct.pack("<d", birth) + extra)


def synthetic_spl(name="Trofeo de prueba"):
    # Cabecera por etiquetas; el organizador coincide con el club de un corredor ("ORCA").
    head = header(name, "Club ORCA", 46298.0, classes=1)  # 2026-10-03
    cls = klass(1, "F21A", (32736, 31, 45, 32752))
    nine = 9 * 3600 * 100
    r1 = runner("José", "García López", "ORCA", 77, 101, 2000123,
                [(32736, nine), (31, nine + 9000), (45, nine + 15000), (32752, nine + 18000)],
                0, 1, 30000.5)
    # El último registro acaba en una etiqueta sin valor (truncado), que su 0x80 sí cuenta.
    r2 = runner("Ana", "Pérez", "Montaña Club", 88, 0, 2000456,
                [(32736, nine), (31, ref.MISSING), (45, nine + 21000), (32752, nine + 24000)],
                6, 0, 31000.0, extra=u8(0x9A, 2))
    return head + cls + r1 + r2[:-1]


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
            for k in ("punches", "status", "place", "sex"):
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

    def test_rewrites_only_the_organizer_in_the_header(self):
        self.assertEqual(self.parsed["event"], {"name": "Trofeo de prueba",
                                                "organizer": "Club CA__", "country": "ESP",
                                                "date": "2026-10-03"})

    def test_check_rejects_other_header_changes(self):
        # Un club en el nombre de la prueba también se reescribe, pero la comprobación lo
        # detecta: hay que revisarlo a mano.
        original = synthetic_spl("Trofeo ORCA")
        with self.assertRaisesRegex(AssertionError, "organizador"):
            anon.check(original, anon.anonymize(original))

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
