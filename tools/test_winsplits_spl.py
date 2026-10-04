"""Tests del lector de referencia (tools/reference/winsplits_spl.py): cabecera por etiquetas.

Los .spl de estos tests son sintéticos. Las funciones de construcción las usa también
test_anonimizar_spl.py.

Ejecutar: python3 -m unittest discover -s tools -p "test_*.py"
"""
import os
import struct
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "reference"))
import winsplits_spl as ref  # noqa: E402

BALTANAS = os.path.join(HERE, "..", "fixtures", "spl", "baltanas-anon.spl")

# --- Construcción de ficheros sintéticos ---

PREAMBLE = b"spl4" + bytes([0x10, 0xDC]) + bytes(6)


def text(tag, s):
    raw = s.encode("latin-1")
    return bytes([tag]) + struct.pack("<H", len(raw)) + raw


def u8(tag, v):
    return bytes([tag, v])


def u16(tag, v):
    return bytes([tag]) + struct.pack("<H", v)


def u32(tag, v):
    return bytes([tag]) + struct.pack("<I", v)


def f64(tag, v):
    return bytes([tag]) + struct.pack("<d", v)


def header(name="Trofeo de prueba", organizer="Club Organizador", date=46298.0, classes=1):
    """Cabecera completa, con las etiquetas en el orden de los ficheros reales.

    `date` en días OLE (46298.0 = 2026-10-03). La tabla 0x20 va a ceros: el lector no la valida.
    """
    return (PREAMBLE + text(0x14, name) + text(0x18, organizer) + text(0x1B, "ESP")
            + f64(0x19, date) + f64(0x22, date + 0.58) + text(0x23, "WinSplits Online Upload 4.0")
            + f64(0x24, date + 0.59) + text(0x25, "WinSplits Online Upload 4.0")
            + text(0x26, "IOFXML3 / Programa de prueba") + u8(0x27, 3) + u32(0x28, 1234)
            + u32(0x29, 0) + u16(0x1F, classes) + u16(0x2B, 0) + u32(0x21, 0)
            + bytes([0x20]) + bytes(8 * classes) + bytes([0x2C]))


def legs(codes):
    """Tramos (0x47) a partir de la secuencia completa salida, balizas…, meta."""
    raw = b"".join(struct.pack("<HHI", a, b, 0) for a, b in zip(codes, codes[1:]))
    return bytes([0x47]) + struct.pack("<I", len(raw)) + raw


def klass(cid, name="F21A", codes=(32736, 31, 45, 32752)):
    return u32(0x40, cid) + text(0x43, name) + legs(list(codes))


def runner_record(body):
    """Registro de corredor: 0x80 con la longitud de `body` y luego `body`."""
    return u32(0x80, len(body)) + body


# --- Tests ---


class HeaderTest(unittest.TestCase):
    def test_date_is_read_by_tag_not_at_a_fixed_offset(self):
        # Nombre más corto que el de Baltanás: la fecha no cae en 0x4C (como en el fichero de
        # Soria que hizo fallar la lectura por offset fijo).
        data = header("Liga de prueba 2026", "Club X", 46222.0, classes=2) + klass(1) + klass(2)
        self.assertNotEqual(struct.unpack_from("<d", data, 0x4C)[0], 46222.0)
        res = ref.parse(data)
        self.assertEqual(res["event"], {"name": "Liga de prueba 2026", "organizer": "Club X",
                                        "country": "ESP", "date": "2026-07-19"})
        self.assertEqual([c["id"] for c in res["classes"]], [1, 2])

    def test_header_ends_at_marker(self):
        h = header(classes=1)
        event, start, count = ref.parse_header(h + klass(7))
        self.assertEqual((start, count), (len(h), 1))
        self.assertEqual(event["date"], "2026-10-03")

    def test_minimal_header(self):
        data = PREAMBLE + f64(0x19, 46298.0) + bytes([0x2C]) + klass(7)
        self.assertEqual(ref.parse(data)["event"], {"date": "2026-10-03"})

    def test_unknown_header_tag_reports_position(self):
        data = PREAMBLE + text(0x14, "Trofeo") + u16(0x15, 0) + f64(0x19, 46298.0) + b"\x2c"
        with self.assertRaisesRegex(ValueError, "etiqueta desconocida 0x15 en el byte 21$"):
            ref.parse(data + klass(7))

    def test_missing_date_is_an_error(self):
        data = PREAMBLE + text(0x14, "Trofeo") + bytes([0x2C]) + klass(7)
        with self.assertRaisesRegex(ValueError, "0x19"):
            ref.parse(data)

    def test_missing_end_marker_is_an_error(self):
        with self.assertRaisesRegex(ValueError, "0x2c"):
            ref.parse(PREAMBLE + f64(0x19, 46298.0))

    def test_class_table_needs_count_first(self):
        data = PREAMBLE + f64(0x19, 46298.0) + bytes([0x20, 0x2C]) + klass(7)
        with self.assertRaisesRegex(ValueError, "0x20"):
            ref.parse(data)

    def test_class_count_must_match(self):
        with self.assertRaisesRegex(ValueError, "anuncia 2 categorías"):
            ref.parse(header(classes=2) + klass(7))

    def test_body_must_start_with_a_class(self):
        with self.assertRaisesRegex(ValueError, "0x40"):
            ref.parse(header() + u8(0x48, 0) + klass(7))


class RunnerLengthTest(unittest.TestCase):
    BODY = text(0x87, "Ana") + u8(0x98, 0) + u8(0x9A, 2) + f64(0x9B, 30000.0)

    def spl(self, first_len=None, last_len=None):
        def rec(n):
            return u32(0x80, len(self.BODY) if n is None else n) + self.BODY
        return header(classes=2) + klass(1) + rec(first_len) + klass(2) + rec(last_len)

    def test_records_that_match_are_read_without_id(self):
        res = ref.parse(self.spl())
        self.assertEqual([r["given"] for c in res["classes"] for r in c["runners"]],
                         ["Ana", "Ana"])
        self.assertNotIn("id", res["classes"][0]["runners"][0])

    def test_mismatch_is_an_error(self):
        n = len(self.BODY)
        for first in (n - 1, n + 1):
            with self.assertRaisesRegex(ValueError, f"declara {first} bytes .* ocupa {n}$"):
                ref.parse(self.spl(first_len=first))
        with self.assertRaisesRegex(ValueError, f"declara {n - 1} bytes .* ocupa {n}$"):
            ref.parse(self.spl(last_len=n - 1))

    def test_last_record_may_be_truncated(self):
        # Como en Baltanás: faltan el valor de 0x9a y el 0x9b entero, que el 0x80 sí cuenta.
        data = self.spl()[:-10]
        self.assertEqual(data[-1], 0x9A)
        last = ref.parse(data)["classes"][1]["runners"][0]
        self.assertEqual(last, {"given": "Ana", "status": 0})


class BaltanasHeaderTest(unittest.TestCase):
    def test_fixture_header(self):
        with open(BALTANAS, "rb") as f:
            data = f.read()
        event, start, count = ref.parse_header(data)
        self.assertEqual(event, {"name": "Cto. SPRINT Liga Norte/Liga FOCYL Baltanas",
                                 "organizer": "Club CH__", "country": "ESP",
                                 "date": "2026-10-03"})
        # En Baltanás la fecha cae en 0x4C (de ahí el antiguo offset fijo).
        self.assertEqual(data[0x4B], 0x19)
        self.assertEqual((start, count), (0x177, 18))
        # 0x21 = posición del primer registro de categoría + 1.
        self.assertEqual(data[0xE0], 0x21)
        self.assertEqual(struct.unpack_from("<I", data, 0xE1)[0], start + 1)
        # La tabla 0x20 da el desplazamiento de cada categoría desde el primer registro.
        self.assertEqual(data[0xE5], 0x20)
        table = 0xE6
        offsets = [struct.unpack_from("<I", data, table + 8 * i)[0] for i in range(count)]
        self.assertTrue(all(data[start + o] == 0x40 for o in offsets))
        self.assertEqual(len(ref.parse(data)["classes"]), 18)


if __name__ == "__main__":
    unittest.main()
