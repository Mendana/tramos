"""Implementación de referencia del tiempo perdido (docs/tiempo-perdido.md).

Oráculo para el test de paridad de `tramos_core::lost_time`. Se escribe a partir del documento,
no del código Rust: si los dos discrepan, uno de los dos (o el documento) está mal.

Entrada: la salida de `winsplits_spl.parse` (categorías con `course` = destinos de los tramos con
la meta al final, corredores con `status`, `place` y `punches` en hora local del día en segundos).
Salida: el mismo JSON que serializa `tramos_core::lost_time::LostTimeReport`, más una clave
`resumen` con un resumen legible para revisión humana (el test de Rust la ignora).

Uso:
  python tiempo_perdido.py carrera.spl [--umbral-s 15] [--umbral-pct 10] [--resumen M-SEN]
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import winsplits_spl  # noqa: E402

START, FINISH = 32736, 32752
DEFAULT_THRESHOLD_S = 15.0
DEFAULT_THRESHOLD_PCT = 10.0
SHORT_REFERENCE_S = 20.0
MIN_STRONG_RUNNERS = 4
DECIMALS = 6  # decimales de los flotantes en el JSON de salida
STATUS = {0: "ok", 6: "not_classified", 10: "did_not_start"}


def status_json(code):
    """Código de estado del .spl → `RaceStatus` en JSON (docs/formato-spl.md)."""
    return STATUS[code] if code in STATUS else {"unknown": code}


def centis(t):
    """Hora del día en segundos → centésimas enteras (las horas del .spl son centésimas)."""
    return None if t is None else round(t * 100)


def match_punches(codes, punches):
    """Hora (en centésimas) de cada baliza esperada, emparejando en orden.

    Para cada código esperado se busca la siguiente picada con ese código a partir de la última
    emparejada; si no aparece, esa baliza queda sin hora y la búsqueda sigue desde el mismo sitio.
    """
    times, pos = [], 0
    for code in codes:
        found = None
        for j in range(pos, len(punches)):
            if punches[j]["code"] == code:
                found = j
                break
        if found is None:
            times.append(None)
        else:
            times.append(centis(punches[found]["time_of_day_s"]))
            pos = found + 1
    return times


def diff_s(a, b):
    """Diferencia b − a en segundos a partir de centésimas, o None si falta alguna."""
    if a is None or b is None:
        return None
    return (b - a) / 100


def weighted_median(pairs):
    """Mediana ponderada de [(valor, peso, tramo)].

    Se ordena por (valor, tramo) y se acumulan los pesos: es el primer valor cuyo peso acumulado
    supera la mitad del total; si el acumulado iguala exactamente la mitad (tolerancia relativa
    1e-9), la media de ese valor y el siguiente. Con pesos iguales es la mediana de siempre.
    """
    if not pairs:
        return None
    pairs = sorted(pairs, key=lambda p: (p[0], p[2]))
    total = 0.0
    for _, w, _ in pairs:
        total += w
    half = total / 2
    eps = total * 1e-9
    cum = 0.0
    for k, (x, w, _) in enumerate(pairs):
        cum += w
        if cum > half + eps:
            return x
        if cum >= half - eps:
            return (x + pairs[k + 1][0]) / 2 if k + 1 < len(pairs) else x
    return pairs[-1][0]


def analyze_course(course, classes, threshold_s, threshold_pct):
    """`course`: balizas sin salida ni meta. `classes`: [(índice, categoría del parser)]."""
    codes = [START] + list(course) + [FINISH]
    n_legs = len(codes) - 1

    entries = []
    for class_index, cls in classes:
        for result_index, r in enumerate(cls["runners"]):
            times = match_punches(codes, r.get("punches", []))
            splits = []
            for i in range(n_legs):
                s = diff_s(times[i], times[i + 1])
                splits.append(s if s is not None and s > 0 else None)
            entries.append({
                "class_index": class_index, "result_index": result_index,
                "status": r["status"], "place": r.get("place") or None,
                "times": times, "splits": splits, "raw": r,
            })

    ok = [e for e in entries if e["status"] == 0]
    legs, refs, ok_splits = [], [], []
    for i in range(n_legs):
        valid = sorted(e["splits"][i] for e in ok if e["splits"][i] is not None)
        ok_splits.append(valid)
        k = (len(valid) + 3) // 4  # 25 % redondeando hacia arriba
        if valid:
            acc = 0.0
            for s in valid[:k]:
                acc += s
            ref = acc / k
        else:
            ref = None
        refs.append(ref)
        short = ref is not None and ref < SHORT_REFERENCE_S
        is_last = i == n_legs - 1
        legs.append({
            "index": i + 1, "from": codes[i], "to": codes[i + 1],
            "valid_splits": len(valid), "reference_count": k, "reference_s": ref,
            "is_last": is_last, "short_reference": short,
            "excluded_from_patterns": is_last or short,
        })

    # Tiempo ideal acumulado: suma de referencias hasta cada tramo (None desde el primer hueco).
    ideal, acc = [], 0.0
    for ref in refs:
        acc = None if acc is None or ref is None else acc + ref
        ideal.append(acc)

    runners = []
    for e in entries:
        irs = [None] * n_legs
        for i in range(n_legs):
            if refs[i] is not None and e["splits"][i] is not None:
                irs[i] = refs[i] / e["splits"][i]
        usual = weighted_median([(irs[i], refs[i], i) for i in range(n_legs) if irs[i] is not None])
        start = e["times"][0]
        out_legs, lost, n_err = [], (0.0 if usual is not None else None), 0
        for i in range(n_legs):
            t = e["splits"][i]
            elapsed = diff_s(start, e["times"][i + 1])
            place = None
            if t is not None:
                place = 1 + sum(1 for s in ok_splits[i] if s < t)
            expected = loss = pct = None
            is_error = False
            if irs[i] is not None and usual is not None:
                expected = refs[i] / usual
                loss = t - expected
                pct = loss / expected * 100
                is_error = loss > threshold_s and pct > threshold_pct
                if is_error:
                    lost += loss
                    n_err += 1
            behind = None
            if elapsed is not None and ideal[i] is not None:
                behind = elapsed - ideal[i]
            out_legs.append({
                "index": i + 1, "split_s": t, "elapsed_s": elapsed, "place": place,
                "performance_index": irs[i], "expected_s": expected, "loss_s": loss,
                "loss_pct": pct, "is_error": is_error, "behind_ideal_s": behind,
            })
        total = diff_s(start, e["times"][-1])
        runners.append({
            "class_index": e["class_index"], "result_index": e["result_index"],
            "status": status_json(e["status"]), "place": e["place"],
            "total_s": total, "usual_performance": usual, "lost_time_s": lost,
            "error_count": n_err,
            "time_without_errors_s": total - lost if total is not None and lost is not None
            else None,
            "legs": out_legs,
        })

    return {
        "course": {"controls": list(course)},
        "classes": [{"index": ci, "id": c["id"], "name": c["name"]} for ci, c in classes],
        "valid_runners": len(ok),
        "weak_reference": len(ok) < MIN_STRONG_RUNNERS,
        "legs": legs,
        "runners": runners,
    }


def group_by_course(parsed):
    """Categorías con la misma secuencia exacta de balizas, en orden de primera aparición."""
    groups, order = {}, []
    for index, cls in enumerate(parsed["classes"]):
        course = [c for c in cls.get("course", [])]
        if course and course[-1] == FINISH:
            course = course[:-1]
        key = tuple(course)
        if key not in groups:
            groups[key] = []
            order.append(key)
        groups[key].append((index, cls))
    return [(list(key), groups[key]) for key in order]


def analyze(parsed, threshold_s=DEFAULT_THRESHOLD_S, threshold_pct=DEFAULT_THRESHOLD_PCT):
    return {
        "config": {"error_threshold_s": threshold_s, "error_threshold_pct": threshold_pct},
        "courses": [analyze_course(course, classes, threshold_s, threshold_pct)
                    for course, classes in group_by_course(parsed)],
    }


def _f(x, nd=1):
    return "—" if x is None else f"{x:.{nd}f}"


def summary(parsed, report, class_name):
    """Resumen legible del recorrido de `class_name`: referencias y cuatro corredores."""
    for course in report["courses"]:
        if any(c["name"] == class_name for c in course["classes"]):
            break
    else:
        raise ValueError(f"no hay categoría {class_name}")
    lines = [
        f"Recorrido de {', '.join(c['name'] for c in course['classes'])}: "
        f"{len(course['course']['controls'])} balizas, {len(course['legs'])} tramos, "
        f"{course['valid_runners']} clasificados"
        + (" (referencia débil)" if course["weak_reference"] else ""),
        "Referencias (s) por tramo; [U] último tramo, [C] referencia < 20 s:",
    ]
    for leg in course["legs"]:
        marks = ("[U]" if leg["is_last"] else "") + ("[C]" if leg["short_reference"] else "")
        lines.append(f"  T{leg['index']:>2} {leg['from']}→{leg['to']}: ref {_f(leg['reference_s'])}"
                     f" (media de {leg['reference_count']} de {leg['valid_splits']}) {marks}".rstrip())
    ok = sorted((r for r in course["runners"] if r["place"] is not None),
                key=lambda r: r["place"])
    chosen = ok[:3] + ok[-1:] if len(ok) > 3 else ok
    for r in chosen:
        raw = parsed["classes"][r["class_index"]]["runners"][r["result_index"]]
        name = f"{raw.get('given', '')} {raw.get('family', '')}".strip()
        lines.append(
            f"Puesto {r['place']} ({name}, tarjeta {raw.get('si_card')}): total {_f(r['total_s'], 0)} s,"
            f" habitual {_f(r['usual_performance'] * 100 if r['usual_performance'] else None)} %,"
            f" {r['error_count']} errores, tiempo perdido {_f(r['lost_time_s'])} s,"
            f" sin errores {_f(r['time_without_errors_s'])} s")
        for leg in r["legs"]:
            lines.append(
                f"    T{leg['index']:>2}: split {_f(leg['split_s'], 0)} (puesto {leg['place']}),"
                f" IR {_f(leg['performance_index'] * 100 if leg['performance_index'] else None)} %,"
                f" esperado {_f(leg['expected_s'])}, pérdida {_f(leg['loss_s'])} s"
                f" ({_f(leg['loss_pct'])} %){' ERROR' if leg['is_error'] else ''},"
                f" tras ideal {_f(leg['behind_ideal_s'])} s")
    return lines


def dumps(obj, indent=0):
    """JSON con los objetos y listas planos (sin anidamiento) en una sola línea.

    Los flotantes se redondean a `DECIMALS` decimales: el test de Rust compara con tolerancia.
    """
    def flat(o):
        if isinstance(o, list) and len(o) > 1 and all(isinstance(v, str) for v in o):
            return False  # líneas de texto (el resumen): una por línea
        return not any(isinstance(v, (dict, list)) for v in (o.values() if isinstance(o, dict) else o))

    if isinstance(obj, float):
        obj = round(obj, DECIMALS)
    if isinstance(obj, (dict, list)) and flat(obj):
        obj = ({k: round(v, DECIMALS) if isinstance(v, float) else v for k, v in obj.items()}
               if isinstance(obj, dict)
               else [round(v, DECIMALS) if isinstance(v, float) else v for v in obj])
    if not isinstance(obj, (dict, list)) or flat(obj):
        return json.dumps(obj, ensure_ascii=False)
    pad, inner = " " * indent, " " * (indent + 1)
    if isinstance(obj, dict):
        items = [f"{inner}{json.dumps(k, ensure_ascii=False)}: {dumps(v, indent + 1)}"
                 for k, v in obj.items()]
        return "{\n" + ",\n".join(items) + "\n" + pad + "}"
    items = [inner + dumps(v, indent + 1) for v in obj]
    return "[\n" + ",\n".join(items) + "\n" + pad + "]"


def _arg(name, default):
    if name in sys.argv:
        return sys.argv[sys.argv.index(name) + 1]
    return default


if __name__ == "__main__":
    parsed = winsplits_spl.parse(open(sys.argv[1], "rb").read())
    report = analyze(parsed, float(_arg("--umbral-s", DEFAULT_THRESHOLD_S)),
                     float(_arg("--umbral-pct", DEFAULT_THRESHOLD_PCT)))
    report["resumen"] = summary(parsed, report, _arg("--resumen", "M-SEN"))
    sys.stdout.write(dumps(report) + "\n")
