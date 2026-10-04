"""Anonimizador de ficheros .spl de WinSplits (cabecera "spl4").

Reescribe en el propio binario los datos personales de los corredores, sin cambiar la longitud
de ningún campo (el fichero conserva tamaño y desplazamientos):

  nombre (0x87) y apellidos (0x88)  -> "Nombre<n>" / "Apellido<n>", misma longitud
  club (0x8c)                        -> "Club A", "Club B"… (uno por club distinto)
  id del club (0x89)                 -> índice del club (1, 2…)
  dorsal (0x81) y tarjeta SI (0x84)  -> número secuencial del corredor (0 se queda en 0)
  fecha de nacimiento (0x9b)         -> 0

No cambian tiempos, códigos, categorías, estados, puestos, sexo ni país. La cabecera (nombre de
la prueba, software) es información pública del evento y se conserva, salvo los nombres de
clubes de corredores que aparezcan en ella (el organizador, 0x18), que pasan a su etiqueta.

Al terminar comprueba con el lector de referencia que la estructura y los tiempos son idénticos,
que la cabecera solo cambia en el organizador y que ningún nombre, apellido o club original
queda en el fichero. Si un club aparece en otro texto de la cabecera (p. ej. el nombre de la
prueba), la comprobación falla y hay que revisarlo a mano.

Uso:
  python tools/anonimizar_spl.py fixtures/private/carrera.spl fixtures/spl/carrera-anon.spl
  (escribe también fixtures/spl/carrera-anon.expected.json con la salida del lector)
"""
import json
import os
import struct
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "reference"))
import winsplits_spl as ref  # noqa: E402

PAD = "_"
DIGITS36 = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"


def _base36(n):
    s = ""
    while True:
        n, r = divmod(n, 36)
        s = DIGITS36[r] + s
        if n == 0:
            return s


def _letters(n):
    """0 -> A, 25 -> Z, 26 -> AA…"""
    s = ""
    n += 1
    while n:
        n, r = divmod(n - 1, 26)
        s = chr(ord("A") + r) + s
    return s


def fit(candidates, length):
    """Primer candidato que cabe en `length` bytes, rellenado con PAD hasta esa longitud."""
    for c in candidates:
        if len(c) <= length:
            return c + PAD * (length - len(c))
    return candidates[-1][-length:] if length else ""


def pseudonym(prefix, seq, length):
    dec, b36 = str(seq), _base36(seq)
    return fit([prefix + dec, prefix[0] + dec, dec, prefix[0] + b36, b36], length)


def club_label(index, length):
    tag = _letters(index)
    return fit(["Club " + tag, "Club" + tag, "C" + tag, tag], length)


def records(data):
    """Recorre los registros (tras la cabecera) como el lector de referencia.

    Devuelve (etiqueta, offset del valor).
    """
    p = ref.parse_header(data)[1]
    out = []
    while p < len(data):
        tag = data[p]
        p += 1
        if p >= len(data):
            break  # registro final truncado
        out.append((tag, p))
        if tag in ref.STR:
            p += 2 + struct.unpack_from("<H", data, p)[0]
        elif tag in ref.U8:
            p += 1
        elif tag in ref.U16:
            p += 2
        elif tag in ref.U32:
            p += 4
        elif tag == 0x9B:
            p += 8
        elif tag == 0x47:
            p += 4 + struct.unpack_from("<I", data, p)[0]
        elif tag == 0x97:
            p += 2 + 5 * struct.unpack_from("<H", data, p)[0]
        else:
            raise ValueError(f"etiqueta desconocida 0x{tag:02x} en el byte {p - 1}")
    return out


def anonymize(data):
    out = bytearray(data)
    seq, clubs, club_ids = 0, {}, {}

    def put_text(off, text):
        n = struct.unpack_from("<H", data, off)[0]
        raw = text.encode("latin-1")
        assert len(raw) == n
        out[off + 2:off + 2 + n] = raw

    for tag, off in records(data):
        if tag == 0x80:
            seq += 1
        elif tag in (0x87, 0x88):
            n = struct.unpack_from("<H", data, off)[0]
            put_text(off, pseudonym("Nombre" if tag == 0x87 else "Apellido", seq, n))
        elif tag == 0x8C:
            n = struct.unpack_from("<H", data, off)[0]
            name = data[off + 2:off + 2 + n].decode("latin-1")
            idx = clubs.setdefault(name, len(clubs))
            put_text(off, club_label(idx, n))
        elif tag == 0x89:
            cid = struct.unpack_from("<I", data, off)[0]
            new = 0 if cid == 0 else club_ids.setdefault(cid, len(club_ids) + 1)
            struct.pack_into("<I", out, off, new)
        elif tag in (0x81, 0x84):
            if struct.unpack_from("<I", data, off)[0] != 0:
                struct.pack_into("<I", out, off, seq)
        elif tag == 0x9B:
            struct.pack_into("<d", out, off, 0.0)
    # En la cabecera, cualquier aparición del nombre de un club de corredor (el organizador,
    # p. ej. "Club ORCA" frente a "ORCA") se sustituye por su etiqueta.
    header_end = ref.parse_header(data)[1]
    for name, idx in clubs.items():
        raw = name.encode("latin-1")
        at = data.find(raw, 0, header_end)
        while len(raw) >= 3 and at != -1:
            out[at:at + len(raw)] = club_label(idx, len(raw)).encode("latin-1")
            at = data.find(raw, at + 1, header_end)
    return bytes(out)


PERSONAL = ("given", "family", "club", "club_id", "bib", "si_card")
# Campos de la cabecera que el anonimizador reescribe a propósito.
EVENT_REWRITTEN = ("organizer",)


def check(original, anon):
    """Lanza AssertionError si el anonimizado no conserva los datos o filtra alguno personal."""
    assert len(original) == len(anon), "cambió el tamaño del fichero"
    a, b = ref.parse(original, keep_birthdate=True), ref.parse(anon, keep_birthdate=True)
    assert {k: v for k, v in a["event"].items() if k not in EVENT_REWRITTEN} == \
           {k: v for k, v in b["event"].items() if k not in EVENT_REWRITTEN}, \
        "la cabecera cambia fuera del organizador (0x18)"
    assert len(a["classes"]) == len(b["classes"])
    leaked = set()
    for ca, cb in zip(a["classes"], b["classes"]):
        assert {k: v for k, v in ca.items() if k != "runners"} == \
               {k: v for k, v in cb.items() if k != "runners"}, f"categoría {ca.get('name')}"
        assert len(ca["runners"]) == len(cb["runners"])
        for ra, rb in zip(ca["runners"], cb["runners"]):
            assert {k: v for k, v in ra.items() if k not in PERSONAL + ("birthdate",)} == \
                   {k: v for k, v in rb.items() if k not in PERSONAL + ("birthdate",)}, \
                f"corredor {ra.get('bib')} de {ca.get('name')}"
            assert rb.get("birthdate") in (None, "1899-12-30"), "queda la fecha de nacimiento"
            for k in ("given", "family", "club"):
                if len(ra.get(k, "")) >= 3:
                    leaked.add(ra[k])
    # Equivalente a `strings`: ningún nombre, apellido ni club original en todo el fichero.
    found = sorted(s for s in leaked if s.encode("latin-1") in anon)
    assert not found, f"{len(found)} textos originales siguen en el fichero"
    return a


def main(argv):
    if len(argv) != 3:
        print(__doc__)
        return 2
    src, dst = argv[1], argv[2]
    original = open(src, "rb").read()
    anon = anonymize(original)
    parsed = check(original, anon)
    with open(dst, "wb") as f:
        f.write(anon)
    expected = os.path.splitext(dst)[0] + ".expected.json"
    with open(expected, "w", encoding="utf-8") as f:
        json.dump(ref.parse(anon), f, ensure_ascii=False, indent=1)
        f.write("\n")
    runners = sum(len(c["runners"]) for c in parsed["classes"])
    courses = len({tuple(c["course"]) for c in parsed["classes"]})
    print(f"{dst}: {len(parsed['classes'])} categorías, {runners} corredores, "
          f"{courses} recorridos. Comprobación superada.")
    print(f"{expected}: salida del lector de referencia.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
