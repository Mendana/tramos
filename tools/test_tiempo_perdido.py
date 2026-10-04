"""Tests de la implementación de referencia del tiempo perdido (tools/reference/tiempo_perdido.py).

Casos calculados a mano, incluido el ejemplo de docs/tiempo-perdido.md, y comprobación de que el
JSON esperado del fixture está al día con el oráculo.

Ejecutar: python3 -m unittest discover -s tools -p "test_*.py"
"""
import json
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "reference"))
import tiempo_perdido as tp  # noqa: E402
import winsplits_spl  # noqa: E402

FIXTURES = os.path.join(HERE, "..", "fixtures", "spl")


def runner(splits, status=0, place=None, controls=(31,)):
    """Corredor con salida a 10:00:00 y los splits dados (None = picada sin hora)."""
    codes = [tp.START] + list(controls) + [tp.FINISH]
    clock = 36000.0
    punches = [{"code": tp.START, "time_of_day_s": clock}]
    for code, s in zip(codes[1:], splits):
        clock += 60.0 if s is None else s
        punches.append({"code": code, "time_of_day_s": None if s is None else clock})
    return {"id": 1, "status": status, "place": place or 0, "punches": punches}


def parsed(runners, controls=(31,), name="M21"):
    return {"event": {}, "classes": [
        {"id": 100, "name": name, "course": list(controls) + [tp.FINISH], "runners": runners}]}


def doc_example():
    """docs/tiempo-perdido.md: 4 corredores, 2 tramos; tramo 1: 60, 62, 70, 90 s."""
    return parsed([runner([60, 66], place=1), runner([62, 60], place=2),
                   runner([70, 75], place=3), runner([90, 60], place=4)])


class DocExample(unittest.TestCase):
    def setUp(self):
        self.course = tp.analyze(doc_example())["courses"][0]

    def test_reference_and_performance_index(self):
        legs = self.course["legs"]
        self.assertEqual(legs[0]["reference_count"], 1)
        self.assertEqual(legs[0]["reference_s"], 60)
        self.assertEqual(legs[1]["reference_s"], 60)
        self.assertTrue(legs[1]["is_last"] and legs[1]["excluded_from_patterns"])
        d = self.course["runners"][3]
        self.assertAlmostEqual(d["legs"][0]["performance_index"], 0.667, places=3)

    def test_runner_d(self):
        d = self.course["runners"][3]
        self.assertAlmostEqual(d["usual_performance"], 5 / 6)
        self.assertAlmostEqual(d["legs"][0]["expected_s"], 72)
        self.assertAlmostEqual(d["legs"][0]["loss_s"], 18)
        self.assertAlmostEqual(d["legs"][0]["loss_pct"], 25)
        self.assertTrue(d["legs"][0]["is_error"])
        self.assertAlmostEqual(d["legs"][1]["loss_s"], -12)
        self.assertFalse(d["legs"][1]["is_error"])
        self.assertEqual(d["total_s"], 150)
        self.assertAlmostEqual(d["lost_time_s"], 18)
        self.assertAlmostEqual(d["time_without_errors_s"], 132)
        self.assertEqual([leg["place"] for leg in d["legs"]], [4, 1])
        self.assertEqual([leg["behind_ideal_s"] for leg in d["legs"]], [30, 30])


class Rules(unittest.TestCase):
    def test_weighted_median(self):
        wm = tp.weighted_median
        self.assertIsNone(wm([]))
        self.assertEqual(wm([(0.9, 1, 0), (0.7, 1, 1), (1.1, 1, 2)]), 0.9)
        self.assertAlmostEqual(wm([(0.9, 2, 0), (0.7, 2, 1), (1.1, 2, 2), (1.0, 2, 3)]), 0.95)
        self.assertEqual(wm([(1.0, 70, 0), (0.6, 10, 1), (0.8, 10, 2), (1.2, 10, 3)]), 1.0)
        self.assertAlmostEqual(wm([(0.6, 30, 0), (0.8, 20, 1), (1.0, 25, 2), (1.2, 25, 3)]), 0.9)

    def test_quarter_rounded_up(self):
        p = parsed([runner([s, 30]) for s in (50, 80, 54, 70, 60)])
        leg = tp.analyze(p)["courses"][0]["legs"][0]
        self.assertEqual(leg["reference_count"], 2)
        self.assertEqual(leg["reference_s"], 52)

    def test_unclassified_do_not_set_reference(self):
        p = parsed([runner([60, 30]), runner([64, 32]), runner([70, 36]), runner([80, 40]),
                    runner([40, 20], status=6)])
        course = tp.analyze(p)["courses"][0]
        self.assertEqual(course["legs"][0]["reference_s"], 60)
        nc = course["runners"][4]
        self.assertEqual(nc["status"], "not_classified")
        self.assertEqual(nc["usual_performance"], 1.5)
        self.assertEqual(nc["legs"][0]["place"], 1)

    def test_weak_and_short_reference(self):
        controls = (31, 32)
        p = parsed([runner([60, 19, 30], controls=controls), runner([70, 25, 30], controls=controls),
                    runner([80, 22, 30], controls=controls)], controls=controls)
        course = tp.analyze(p)["courses"][0]
        self.assertTrue(course["weak_reference"])
        self.assertEqual([leg["short_reference"] for leg in course["legs"]], [False, True, False])

    def test_thresholds(self):
        controls = (31, 32, 33)

        def case(r4, t4, **kw):
            rs = [runner([300, 300, 300, r4], controls=controls) for _ in range(4)]
            rs.append(runner([300, 300, 300, t4], controls=controls))
            return tp.analyze(parsed(rs, controls), **kw)["courses"][0]["runners"][4]["legs"][3]

        self.assertTrue(case(100, 116)["is_error"])
        self.assertFalse(case(100, 115)["is_error"])
        self.assertFalse(case(400, 420)["is_error"])
        self.assertFalse(case(20, 30)["is_error"])
        self.assertTrue(case(400, 420, threshold_s=5, threshold_pct=4)["is_error"])

    def test_missing_punch(self):
        controls = (31, 32)
        rs = [runner([60, 60, 30], controls=controls) for _ in range(4)]
        rs.append(runner([None, 60, 30], controls=controls))
        x = tp.analyze(parsed(rs, controls))["courses"][0]["runners"][4]
        self.assertEqual([leg["split_s"] for leg in x["legs"]], [None, None, 30])
        self.assertEqual(x["legs"][1]["elapsed_s"], 120)
        self.assertEqual(x["total_s"], 150)

    def test_match_punches_in_order(self):
        punches = [{"code": c, "time_of_day_s": t}
                   for c, t in ((tp.START, 0), (31, 50), (45, 70), (tp.FINISH, 150))]
        self.assertEqual(tp.match_punches([tp.START, 31, 32, tp.FINISH], punches),
                         [0, 5000, None, 15000])


class IdealTime(unittest.TestCase):
    """5 corredores, 2 tramos: tramo 1 en 50, 80, 54, 70 y 60 s; tramo 2 en 30 s todos."""

    def course(self, ideal_time):
        p = parsed([runner([s, 30]) for s in (50, 80, 54, 70, 60)])
        report = tp.analyze(p, ideal_time=ideal_time)
        self.assertEqual(report["config"]["ideal_time"], ideal_time)
        return report["courses"][0]

    def test_sum_of_references_is_default(self):
        self.assertEqual(tp.analyze(doc_example())["config"]["ideal_time"], "sum_of_references")
        course = self.course(tp.SUM_OF_REFERENCES)
        self.assertEqual([leg["ideal_elapsed_s"] for leg in course["legs"]], [52, 82])
        self.assertEqual([leg["behind_ideal_s"] for leg in course["runners"][1]["legs"]], [28, 28])

    def test_sum_of_best_splits(self):
        course = self.course(tp.SUM_OF_BEST_SPLITS)
        self.assertEqual([leg["ideal_elapsed_s"] for leg in course["legs"]], [50, 80])
        self.assertEqual([leg["behind_ideal_s"] for leg in course["runners"][1]["legs"]], [30, 30])
        # El resto no cambia.
        by_refs = self.course(tp.SUM_OF_REFERENCES)
        self.assertEqual(course["legs"][0]["reference_s"], by_refs["legs"][0]["reference_s"])
        self.assertEqual(course["runners"][1]["usual_performance"],
                         by_refs["runners"][1]["usual_performance"])

    def test_best_split_ignores_unclassified(self):
        p = parsed([runner([60, 30]), runner([70, 40]), runner([40, 20], status=6)])
        course = tp.analyze(p, ideal_time=tp.SUM_OF_BEST_SPLITS)["courses"][0]
        self.assertEqual([leg["ideal_elapsed_s"] for leg in course["legs"]], [60, 90])

    def test_unknown_definition_fails(self):
        with self.assertRaises(ValueError):
            tp.analyze(doc_example(), ideal_time="otra")


class Fixture(unittest.TestCase):
    def test_expected_json_is_up_to_date(self):
        with open(os.path.join(FIXTURES, "baltanas-anon.spl"), "rb") as f:
            data = winsplits_spl.parse(f.read())
        report = tp.analyze(data)
        report["resumen"] = tp.summary(data, report, "M-SEN")
        with open(os.path.join(FIXTURES, "baltanas-anon.tiempo-perdido.expected.json"),
                  encoding="utf-8") as f:
            expected = f.read()
        self.assertEqual(tp.dumps(tp.compact(report)) + "\n", expected)
        self.assertEqual(len(json.loads(expected)["courses"]), 9)

    def test_compact_keeps_every_leg_with_a_split(self):
        with open(os.path.join(FIXTURES, "baltanas-anon.spl"), "rb") as f:
            report = tp.analyze(winsplits_spl.parse(f.read()))
        compact = tp.compact(report)
        columns = compact["runner_leg_columns"]
        for course, small in zip(report["courses"], compact["courses"]):
            for r, rs in zip(course["runners"], small["runners"]):
                if rs["legs"]:
                    self.assertEqual([dict(zip(columns, row)) for row in rs["legs"]], r["legs"])
                else:
                    self.assertTrue(all(leg["split_s"] is None for leg in r["legs"]))

    def test_best_splits_on_fixture(self):
        with open(os.path.join(FIXTURES, "baltanas-anon.spl"), "rb") as f:
            data = winsplits_spl.parse(f.read())
        report = tp.analyze(data, ideal_time=tp.SUM_OF_BEST_SPLITS)
        for course in report["courses"]:
            acc = 0.0
            for i, leg in enumerate(course["legs"]):
                acc += min(r["legs"][i]["split_s"] for r in course["runners"]
                           if r["status"] == "ok" and r["legs"][i]["split_s"] is not None)
                self.assertAlmostEqual(leg["ideal_elapsed_s"], acc)
                for r in course["runners"]:
                    e = r["legs"][i]["elapsed_s"]
                    want = None if e is None else e - leg["ideal_elapsed_s"]
                    self.assertEqual(r["legs"][i]["behind_ideal_s"], want)


if __name__ == "__main__":
    unittest.main()
