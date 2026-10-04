"""Tests del generador de FIT sintético (sin dependencias externas).

Ejecutar: python3 -m unittest discover -s tools -p "test_*.py"
"""
import datetime
import os
import struct
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fit_sintetico as gen  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SPL = "fixtures/spl/baltanas-anon.spl"
FIT = "fixtures/fit/baltanas-sintetico.fit"
CARD = 143


def decode(data):
    """Decodificador FIT mínimo (cabeceras normales, sin campos de desarrollador) para los tests.
    Devuelve [(número global, {número de campo: valor})] y comprueba ambos CRC."""
    size, _, _, data_size, magic = struct.unpack_from("<BBHI4s", data, 0)
    assert size == 14 and magic == b".FIT", "cabecera"
    assert gen.crc16(data[:12]) == struct.unpack_from("<H", data, 12)[0], "CRC de cabecera"
    assert len(data) == 14 + data_size + 2, "tamaño"
    assert gen.crc16(data[:-2]) == struct.unpack_from("<H", data, len(data) - 2)[0], "CRC final"
    fmt_of = {code: fmt for code, fmt in gen.BASE_TYPES.values()}
    defs, out, p, end = {}, [], 14, 14 + data_size
    while p < end:
        header = data[p]
        p += 1
        assert header & 0x80 == 0, "cabecera comprimida inesperada"
        local = header & 0x0F
        if header & 0x40:
            _, arch, global_num, n = struct.unpack_from("<BBHB", data, p)
            assert arch == 0
            p += 5
            fields = [struct.unpack_from("<BBB", data, p + 3 * i) for i in range(n)]
            p += 3 * n
            defs[local] = (global_num, [(num, fmt_of[base]) for num, _, base in fields])
        else:
            global_num, fields = defs[local]
            values = {}
            for num, fmt in fields:
                values[num] = struct.unpack_from("<" + fmt, data, p)[0]
                p += struct.calcsize(fmt)
            out.append((global_num, values))
    return out


def fit_time(ts):
    return gen.FIT_EPOCH + datetime.timedelta(seconds=ts)


class CrcTest(unittest.TestCase):
    def test_check_value(self):
        # CRC-16/ARC: valor de comprobación estándar.
        self.assertEqual(gen.crc16(b"123456789"), 0xBB3D)
        self.assertEqual(gen.crc16(b""), 0)

    def test_file_with_crc_appended_has_zero_residue(self):
        w = gen.FitWriter()
        w.write(gen.EVENT, timestamp=1, event=0, event_type=0)
        data = w.finish()
        self.assertEqual(gen.crc16(data[:14]), 0)
        self.assertEqual(gen.crc16(data), 0)


class TimeTest(unittest.TestCase):
    def test_summer_time(self):
        utc = gen.local_to_utc("2026-10-03", 18 * 3600 + 13 * 60)
        self.assertEqual(utc, datetime.datetime(2026, 10, 3, 16, 13, tzinfo=gen.UTC))

    def test_winter_time(self):
        utc = gen.local_to_utc("2026-11-15", 10 * 3600)
        self.assertEqual(utc, datetime.datetime(2026, 11, 15, 9, 0, tzinfo=gen.UTC))

    def test_fit_epoch(self):
        self.assertEqual(gen.fit_timestamp(datetime.datetime(1990, 1, 1, tzinfo=gen.UTC)), 86400)


class GeneratedFitTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cwd = os.getcwd()
        os.chdir(ROOT)
        try:
            cls.fit, cls.truth = gen.build(SPL, FIT, CARD)
        finally:
            os.chdir(cwd)
        messages = decode(cls.fit)
        cls.records = [v for g, v in messages if g == gen.RECORD[0]]
        cls.messages = messages
        cls.times = [fit_time(r[253]) for r in cls.records]
        cls.pos = [(gen.from_semicircles(r[0]), gen.from_semicircles(r[1])) for r in cls.records]

    def index_at(self, iso):
        t = datetime.datetime.strptime(iso, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=gen.UTC)
        return self.times.index(t)

    def test_matches_committed_fixture(self):
        with open(os.path.join(ROOT, FIT), "rb") as f:
            self.assertEqual(f.read(), self.fit, "regenera el fixture: ver fixtures/README.md")
        with open(gen.truth_path(os.path.join(ROOT, FIT)), encoding="utf-8") as f:
            self.assertEqual(f.read(), gen.dump_truth(self.truth))

    def test_message_structure(self):
        kinds = [g for g, _ in self.messages]
        self.assertEqual(kinds[0], gen.FILE_ID[0])
        for msg in (gen.LAP, gen.SESSION, gen.ACTIVITY):
            self.assertEqual(kinds.count(msg[0]), 1)
        self.assertEqual(kinds.count(gen.EVENT[0]), 2)

    def test_runner_and_offset(self):
        r = self.truth["runner"]
        self.assertEqual((r["class_name"], r["si_card"], r["status"], r["place"]),
                         ("M-SEN", 143, 0, 16))
        self.assertEqual(self.truth["utc_offset_s"], 7200)  # 3 de octubre: horario de verano
        self.assertEqual(self.truth["controls"][0]["punch_utc"], "2026-10-03T16:13:00Z")
        self.assertEqual(self.truth["controls"][0]["punch_local"], "2026-10-03T18:13:00+02:00")

    def test_track_length_at_1_hz(self):
        start = self.index_at(self.truth["controls"][0]["punch_utc"])
        finish = self.index_at(self.truth["controls"][-1]["punch_utc"])
        self.assertEqual(start, gen.PRE_START_S)
        self.assertEqual(len(self.records) - 1 - finish, gen.POST_FINISH_S)
        self.assertEqual(finish - start, 67120 - 65580)  # meta - salida en el .spl
        self.assertEqual(len(self.records), self.truth["track"]["records"])
        steps = {(b - a).total_seconds() for a, b in zip(self.times, self.times[1:])}
        self.assertEqual(steps, {1.0})

    def test_passes_each_control_at_its_punch(self):
        controls = self.truth["controls"]
        self.assertEqual(len(controls), 22)  # salida, 20 balizas y meta
        self.assertEqual(len(self.truth["legs"]), 21)
        for c in controls:
            k = self.index_at(c["punch_utc"])
            self.assertLess(gen.haversine_m(self.pos[k], (c["lat"], c["lon"])), 0.05, c["code"])

    def test_straight_legs_within_sprint_range(self):
        for leg in self.truth["legs"]:
            self.assertGreaterEqual(leg["straight_m"], gen.MIN_LEG_M - 0.05)
            self.assertLessEqual(leg["straight_m"], gen.MAX_LEG_M + 0.05)

    def test_stop_lasts_30_s_away_from_punches(self):
        stop = self.truth["stop"]
        k0, k1 = self.index_at(stop["start_utc"]), self.index_at(stop["end_utc"])
        self.assertEqual(k1 - k0, 30)
        self.assertEqual(len({self.pos[k] for k in range(k0, k1 + 1)}), 1)
        self.assertNotEqual(self.pos[k0 - 1], self.pos[k0])
        self.assertNotEqual(self.pos[k1 + 1], self.pos[k1])
        leg = self.truth["legs"][stop["leg"] - 1]
        a = self.index_at(self.truth["controls"][stop["leg"] - 1]["punch_utc"])
        b = self.index_at(self.truth["controls"][stop["leg"]]["punch_utc"])
        self.assertGreaterEqual(k0 - a, gen.PUNCH_MARGIN_S)
        self.assertGreaterEqual(b - k1, gen.PUNCH_MARGIN_S)
        self.assertEqual(b - a, leg["split_s"])
        # Velocidad < 0,5 m/s: exactamente los 30 s de la parada en todo el track de carrera.
        slow = [k for k in range(gen.PRE_START_S + 1, len(self.records) - gen.POST_FINISH_S)
                if gen.haversine_m(self.pos[k - 1], self.pos[k]) < 0.5]
        self.assertEqual(slow, list(range(k0 + 1, k1 + 1)))

    def test_detour_ratio(self):
        d = self.truth["detour"]
        a = self.index_at(self.truth["controls"][d["leg"] - 1]["punch_utc"])
        b = self.index_at(self.truth["controls"][d["leg"]]["punch_utc"])
        path = sum(gen.haversine_m(self.pos[k], self.pos[k + 1]) for k in range(a, b))
        straight = gen.haversine_m(self.pos[a], self.pos[b])
        self.assertAlmostEqual(path, d["path_m"], delta=0.05)
        self.assertAlmostEqual(straight, d["straight_m"], delta=0.05)
        self.assertGreaterEqual(path / straight, gen.DETOUR_MIN_RATIO)
        self.assertEqual(self.truth["stop"]["leg"], d["leg"])
        for leg in self.truth["legs"]:
            if leg["leg"] != d["leg"]:
                self.assertLess(leg["ratio"], gen.DETOUR_MIN_RATIO, leg["leg"])

    def test_plausible_sensors(self):
        hr = [r[3] for r in self.records]
        cadence = [r[4] for r in self.records]
        alt = [r[2] / 5 - 500 for r in self.records]
        speed = [r[6] / 1000 for r in self.records]
        self.assertTrue(all(90 <= h <= 195 for h in hr))
        self.assertTrue(all(c == 0 or 50 <= c <= 100 for c in cadence))
        self.assertTrue(all(760 <= a <= 830 for a in alt))
        self.assertLess(max(speed), 6.5)
        dist = [r[5] / 100 for r in self.records]
        self.assertEqual(dist, sorted(dist))
        self.assertAlmostEqual(dist[-1], self.truth["track"]["total_distance_m"], delta=0.01)


class RunnerSelectionTest(unittest.TestCase):
    def test_rejects_unknown_or_unclassified(self):
        with open(os.path.join(ROOT, SPL), "rb") as f:
            parsed = gen.ref.parse(f.read())
        with self.assertRaises(ValueError):
            gen.find_runner(parsed, 999999)
        dns = next(r for c in parsed["classes"] for r in c["runners"] if r.get("status") == 10)
        with self.assertRaises(ValueError):
            gen.find_runner(parsed, dns["si_card"])

    def test_rejects_short_detour_leg(self):
        with open(os.path.join(ROOT, SPL), "rb") as f:
            parsed = gen.ref.parse(f.read())
        with self.assertRaises(ValueError):
            gen.generate(parsed, CARD, detour_leg=21)  # 17 s: no cabe la parada


if __name__ == "__main__":
    unittest.main()
