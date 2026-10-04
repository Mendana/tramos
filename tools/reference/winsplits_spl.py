"""Lector de referencia para ficheros .spl de WinSplits (cabecera "spl4").

Formato deducido de un fichero real (Liga Madrid MTBO 2026, Chinchón).
No hay especificación pública: si un fichero nuevo falla, el parser se
detiene en la etiqueta desconocida e informa de su posición.

Estructura: secuencia de registros <etiqueta:1 byte><valor>.
  texto  = uint16 longitud + bytes Latin-1
  enteros little-endian; double = 8 bytes IEEE-754

Categoría (empieza con 0x40)
  0x40 u32 id            0x43 txt nombre        0x44 txt nombre corto
  0x45 u16 ?             0x4e u8 ?              0x4f u16 ?   0x50 u16 ?
  0x47 u32 n + n bytes   tramos: (desde u16, hasta u16, longitud u32, a 0 en el ejemplo)
  0x48/0x49/0x4a u8 ?    0x4d u32 ?
Corredor (empieza con 0x80)
  0x80 u32 id            0x81 u32 dorsal?       0x84 u32 tarjeta SportIdent
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


def parse(data, keep_birthdate=False):
    if data[:4] != b"spl4":
        raise ValueError("no es un fichero spl4")
    event = {"date": _ole(struct.unpack_from("<d", data, 0x4C)[0])}
    # El primer registro de categoría es 0x40 seguido de 4 bytes y 0x43 (nombre).
    p = next(i for i in range(0x40, len(data) - 6) if data[i] == 0x40 and data[i + 5] == 0x43)
    classes, cls, runner = [], None, None
    while p < len(data):
        tag = data[p]
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
            runner = {"id": val}
            cls["runners"].append(runner)
        elif runner is not None and tag >= 0x80:
            if val is not None:
                runner[NAMES.get(tag, hex(tag))] = val
        elif tag in NAMES:
            cls[NAMES[tag]] = val
    for c in classes:
        c["course"] = [leg[1] for leg in c.get("legs", [])]
        c.pop("legs", None)
    return {"event": event, "classes": classes}


def legs_rows(result):
    """Una fila por corredor y tramo, con split y tiempo acumulado en segundos."""
    for c in result["classes"]:
        for r in c["runners"]:
            punches = r.get("punches", [])
            start = punches[0]["time_of_day_s"] if punches else None
            for i in range(1, len(punches)):
                a, b = punches[i - 1], punches[i]
                ok = None not in (a["time_of_day_s"], b["time_of_day_s"])
                yield {
                    "class": c.get("name"), "runner_id": r["id"],
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
