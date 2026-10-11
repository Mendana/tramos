"""Saca del CHANGELOG.md la sección de una versión (`## X.Y.Z`).

Lo usa el workflow de release (.github/workflows/release.yml) para poner las notas de la versión
en la release y en `latest.json`. Falla (código 1) si no hay sección para esa versión o si está
vacía, para que no se publique una versión sin notas.

Uso: python3 tools/changelog_seccion.py CHANGELOG.md 0.2.1
"""
import re
import sys


def seccion(texto: str, version: str) -> str:
    """Cuerpo de la sección `## <version>`, sin el título, hasta el siguiente `## `."""
    lineas = texto.replace("\r\n", "\n").split("\n")
    titulo = re.compile(r"^##\s+" + re.escape(version.lstrip("v")) + r"\s*$")
    for i, linea in enumerate(lineas):
        if titulo.match(linea):
            cuerpo = []
            for siguiente in lineas[i + 1 :]:
                if siguiente.startswith("## "):
                    break
                cuerpo.append(siguiente)
            notas = "\n".join(cuerpo).strip()
            if notas == "":
                raise ValueError(f"La sección {version} del CHANGELOG está vacía.")
            return notas
    raise ValueError(f"El CHANGELOG no tiene sección «## {version}».")


def main(argv: list) -> int:
    if len(argv) != 3:
        print("Uso: changelog_seccion.py CHANGELOG.md X.Y.Z", file=sys.stderr)
        return 2
    try:
        with open(argv[1], encoding="utf-8") as f:
            notas = seccion(f.read(), argv[2])
    except (OSError, ValueError) as err:
        print(f"error: {err}", file=sys.stderr)
        return 1
    # Bytes UTF-8 y saltos `\n` también en Windows, para que el fichero sea igual en todas partes.
    sys.stdout.buffer.write((notas + "\n").encode("utf-8"))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
