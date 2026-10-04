"""Lector de referencia para ficheros .spl de WinSplits (cabecera "spl4").

Formato deducido de ficheros reales (Liga Madrid MTBO 2026, Chinchón; sprint de Baltanás;
prueba de Soria). No hay especificación pública: si un fichero nuevo falla, el parser se
detiene en la etiqueta desconocida e informa de su posición. Ver docs/formato-spl.md.

Estructura: secuencia de registros <etiqueta:1 byte><valor>.
  texto  = uint16 longitud + bytes Latin-1
  enteros little-endian; double = 8 bytes IEEE-754; fechas OLE (días desde 1899-12-30)

Cabecera: "spl4" + 8 bytes sin identificar (preámbulo) y registros hasta el marcador 0x2c.
La posición de cada campo depende de la longitud de los textos anteriores: se recorre por
etiquetas, nunca por offsets fijos.
  0x14 txt nombre de la prueba   0x18 txt organizador   0x1b txt país
  0x19 f64 fecha de la carrera (OLE; obligatoria)
  0x22 f64 fecha y hora (¿creación?)  0x23 txt software (p. ej. "WinSplits Online Upload 4.0")
  0x24 f64 fecha y hora (¿subida?)    0x25 txt software
  0x26 txt origen de los resultados (p. ej. "IOFXML3 / SportSoftware OE2010 (M) V.11.0")
  0x27 u8 ? (3)          0x28 u32 ? (¿id del evento en WinSplits Online?)    0x29 u32 ? (0)
  0x1f u16 número de categorías  0x2b u16 ? (0)
  0x21 u32 posición del primer registro de categoría + 1
  0x20 tabla: n x (desplazamiento u32, tamaño u32), n = 0x1f; desplazamientos desde el
       primer registro de categoría, acumulativos
  0x2c fin de cabecera, sin valor: el primer registro de categoría (0x40) va justo detrás
Categoría (empieza con 0x40)
  0x40 u32 id            0x43 txt nombre        0x44 txt nombre corto
  0x45 u16 ?             0x4e u8 ?              0x4f u16 ?   0x50 u16 ?
  0x47 u32 n + n bytes   tramos: (desde u16, hasta u16, longitud u32, a 0 en el ejemplo)
  0x48/0x49/0x4a u8 ?    0x4d u32 ?
Corredor (empieza con 0x80)
  0x80 u32 longitud en bytes del resto del registro (no es un id). Se comprueba que cada
       corredor ocupa lo que declara; solo el último puede quedarse corto si el fichero acaba
       antes (truncado de origen). No sale en el JSON.
                         0x81 u32 dorsal?       0x84 u32 tarjeta SportIdent
  0x87 txt nombre        0x88 txt apellidos     0x89 u32 id club
  0x8c txt club          0x8d txt país          0x8e txt nacionalidad
  0x97 u16 n + n x (código u16, hora u24)        hora = centésimas desde medianoche,
                                                 0xFFFFFF = sin picada
  0x98 u8 estado (0 = clasificado; 10 en los no presentados del ejemplo)
  0x99 u16 puesto        0x9a u8 sexo (1 = M, 2 = F)
  0x9b f64 fecha de nacimiento (fecha OLE); se descarta salvo --keep-birthdate
Códigos especiales: 32736 = salida, 32752 = meta.

Uso:
  python winsplits_spl.py carrera.spl            -> JSON
  python winsplits_spl.py carrera.spl --csv      -> CSV de tramos (una fila por corredor y tramo)
"""
import csv
import datetime
import json
import struct
import sys

START, FINISH, MISSING = 32736, 32752, 0xFFFFFF
PREAMBLE = 12  # "spl4" + 8 bytes sin identificar
HEADER_STR = {0x14: "name", 0x18: "organizer", 0x1B: "country", 0x23: None, 0x25: None, 0x26: None}
HEADER_F64 = {0x19, 0x22, 0x24}
HEADER_U8 = {0x27}
HEADER_U16 = {0x1F, 0x2B}
HEADER_U32 = {0x21, 0x28, 0x29}
CLASS_COUNT, CLASS_TABLE, HEADER_END = 0x1F, 0x20, 0x2C
EVENT_KEYS = ("name", "organizer", "country", "date")
STR = {0x43, 0x44, 0x87, 0x88, 0x8C, 0x8D, 0x8E}
U8 = {0x48, 0x49, 0x4A, 0x4E, 0x98, 0x9A}
U16 = {0x45, 0x4F, 0x50, 0x99}
U32 = {0x40, 0x4D, 0x80, 0x81, 0x84, 0x89}
NAMES = {0x43: "name", 0x44: "short_name", 0x47: "legs", 0x81: "bib", 0x84: "si_card",
         0x87: "given", 0x88: "family", 0x89: "club_id", 0x8C: "club", 0x8D: "country",
         0x8E: "nationality", 0x97: "punches", 0x98: "status", 0x99: "place", 0x9A: "sex",
         0x9B: "birthdate"}


def _ole(days):
    return (datetime.datetime(1899, 12, 30) + datetime.timedelta(days=days)).date().isoformat()


def parse_header(data):
    """Recorre la cabecera por etiquetas.

    Devuelve (evento, posición del primer registro de categoría, número de categorías o None).
    """
    if data[:4] != b"spl4":
        raise ValueError("no es un fichero spl4")
    event, count, p = {}, None, PREAMBLE
    while True:
        if p >= len(data):
            raise ValueError("la cabecera termina sin el marcador de fin (0x2c)")
        tag = data[p]
        p += 1
        if tag == HEADER_END:
            break
        if tag in HEADER_STR:
            n = struct.unpack_from("<H", data, p)[0]
            val, p = data[p + 2:p + 2 + n].decode("latin-1"), p + 2 + n
            if HEADER_STR[tag]:
                event[HEADER_STR[tag]] = val
        elif tag in HEADER_F64:
            val, p = struct.unpack_from("<d", data, p)[0], p + 8
            if tag == 0x19:
                event["date"] = _ole(val)
        elif tag in HEADER_U8:
            p += 1
        elif tag in HEADER_U16:
            val, p = struct.unpack_from("<H", data, p)[0], p + 2
            if tag == CLASS_COUNT:
                count = val
        elif tag in HEADER_U32:
            p += 4
        elif tag == CLASS_TABLE:
            if count is None:
                raise ValueError(f"tabla de categorías (0x20) antes de su número (0x1f) "
                                 f"en el byte {p - 1}")
            p += 8 * count
        else:
            raise ValueError(f"etiqueta desconocida 0x{tag:02x} en el byte {p - 1}")
    if "date" not in event:
        raise ValueError("falta la fecha de la carrera (0x19) en la cabecera")
    return {k: event[k] for k in EVENT_KEYS if k in event}, p, count


def _check_runner_len(open_runner, end, at_eof):
    """Comprueba que el corredor abierto (byte de su 0x80, inicio del resto, longitud declarada)
    acaba en `end`. Al final del fichero se admite que se quede corto."""
    if open_runner is None:
        return
    offset, body, declared = open_runner
    actual = end - body
    if actual != declared and not (at_eof and actual < declared):
        raise ValueError(f"el corredor del byte {offset} declara {declared} bytes de registro "
                         f"(0x80) y ocupa {actual}")


def parse(data, keep_birthdate=False):
    event, p, count = parse_header(data)
    if p >= len(data) or data[p] != 0x40:
        raise ValueError(f"no hay registro de categoría (0x40) tras la cabecera, byte {p}")
    classes, cls, runner, open_runner = [], None, None, None
    while p < len(data):
        tag = data[p]
        if tag in (0x40, 0x80):
            _check_runner_len(open_runner, p, at_eof=False)
            open_runner = None
        p += 1
        if p >= len(data):
            break  # registro final truncado
        if tag in STR:
            n = struct.unpack_from("<H", data, p)[0]
            val, p = data[p + 2:p + 2 + n].decode("latin-1"), p + 2 + n
        elif tag in U8:
            val, p = data[p], p + 1
        elif tag in U16:
            val, p = struct.unpack_from("<H", data, p)[0], p + 2
        elif tag in U32:
            val, p = struct.unpack_from("<I", data, p)[0], p + 4
        elif tag == 0x9B:
            val, p = struct.unpack_from("<d", data, p)[0], p + 8
            val = _ole(val) if keep_birthdate else None
        elif tag == 0x47:
            n = struct.unpack_from("<I", data, p)[0]
            raw, p = data[p + 4:p + 4 + n], p + 4 + n
            val = [struct.unpack_from("<HHI", raw, i) for i in range(0, n, 8)]
        elif tag == 0x97:
            n = struct.unpack_from("<H", data, p)[0]
            p += 2
            val = []
            for _ in range(n):
                code = struct.unpack_from("<H", data, p)[0]
                cs = int.from_bytes(data[p + 2:p + 5], "little")
                val.append({"code": code, "time_of_day_s": None if cs == MISSING else cs / 100})
                p += 5
        else:
            raise ValueError(f"etiqueta desconocida 0x{tag:02x} en el byte {p - 1}")
        if tag == 0x40:
            cls = {"id": val, "runners": []}
            classes.append(cls)
            runner = None
        elif tag == 0x80:
            runner, open_runner = {}, (p - 5, p, val)
            cls["runners"].append(runner)
        elif runner is not None and tag >= 0x80:
            if val is not None:
                runner[NAMES.get(tag, hex(tag))] = val
        elif tag in NAMES:
            cls[NAMES[tag]] = val
    _check_runner_len(open_runner, len(data), at_eof=True)
    if count is not None and len(classes) != count:
        raise ValueError(f"la cabecera anuncia {count} categorías (0x1f) y hay {len(classes)}")
    for c in classes:
        c["course"] = [leg[1] for leg in c.get("legs", [])]
        c.pop("legs", None)
    return {"event": event, "classes": classes}


def legs_rows(result):
    """Una fila por corredor y tramo, con split y tiempo acumulado en segundos."""
    for c in result["classes"]:
        for index, r in enumerate(c["runners"]):
            punches = r.get("punches", [])
            start = punches[0]["time_of_day_s"] if punches else None
            for i in range(1, len(punches)):
                a, b = punches[i - 1], punches[i]
                ok = None not in (a["time_of_day_s"], b["time_of_day_s"])
                yield {
                    "class": c.get("name"), "index_in_class": index,
                    "runner": f"{r.get('given', '')} {r.get('family', '')}".strip(),
                    "club": r.get("club"), "status": r.get("status"), "place": r.get("place"),
                    "leg": i, "from": a["code"], "to": b["code"],
                    "split_s": round(b["time_of_day_s"] - a["time_of_day_s"]) if ok else None,
                    "elapsed_s": round(b["time_of_day_s"] - start)
                    if start is not None and b["time_of_day_s"] is not None else None,
                    "punch_time_of_day_s": b["time_of_day_s"],
                }


if __name__ == "__main__":
    res = parse(open(sys.argv[1], "rb").read(), keep_birthdate="--keep-birthdate" in sys.argv)
    if "--csv" in sys.argv:
        rows = list(legs_rows(res))
        w = csv.DictWriter(sys.stdout, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)
    else:
        json.dump(res, sys.stdout, ensure_ascii=False, indent=1)
