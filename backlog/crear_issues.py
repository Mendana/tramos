#!/usr/bin/env python3
"""Crea en GitHub las etiquetas, milestones e issues de backlog/issues/ usando la CLI `gh`.

Uso:
  gh auth login                                  # una vez
  python3 backlog/crear_issues.py --repo usuario/tramos --dry-run   # ver qué haría
  python3 backlog/crear_issues.py --repo usuario/tramos

Es reanudable: guarda en backlog/.creadas.json qué issue de GitHub corresponde a cada id (B01…),
así que si falla a mitad, al relanzarlo no duplica. Las referencias "B08" de "Depende de" se
convierten en "#<número>" reales.
"""
import argparse
import heapq
import json
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent
ISSUES = ROOT / "issues"
STATE = ROOT / ".creadas.json"

MILESTONES = {
    "M0 Cimientos": "Repo, CI, fixtures públicos y modelo de dominio",
    "M1 Núcleo de datos": "Lectores, alineación, segmentación, tiempo perdido y CLI",
    "M2 Esqueleto de la app": "App Tauri, almacenamiento, importación, tabla y mapa",
    "M3 Tanda 1: splits": "P1, P3, P4, P5, P6, P10",
    "M4 Tanda 2: FIT": "P2, P7, P13",
    "M5 Tanda 3: comportamiento y etiquetas": "Etiquetado, P8, P9, P11, P14",
    "M6 Tanda 4: entrenadora": "Paquetes, carpeta compartida, modo entrenadora y P15",
    "M7 Distribución": "Instalador de Windows y actualizaciones",
}
LABEL_COLORS = {"area:": "1d76db", "tipo:": "bfd4f2", "tanda-": "0e8a16",
                "listo-para-agente": "2ea44f", "necesita-humano": "d93f0b"}
LABEL_DESC = {"listo-para-agente": "Bien especificada; revisa sus dependencias antes de lanzarla",
              "necesita-humano": "Requiere datos reales, una decisión o probar en Windows"}


def parse(path):
    text = path.read_text(encoding="utf-8")
    m = re.match(r"---\n(.*?)\n---\n(.*)", text, re.S)
    if not m:
        sys.exit(f"Sin cabecera: {path}")
    meta = dict(line.split(": ", 1) if ": " in line else (line.rstrip(":"), "")
                for line in m.group(1).splitlines())
    split = lambda v: [x.strip() for x in v.split(",") if x.strip()]
    return {"id": meta["id"], "title": meta["title"], "milestone": meta["milestone"],
            "labels": split(meta.get("labels", "")), "depends": split(meta.get("depends", "")),
            "body": m.group(2)}


def topo(issues):
    by_id = {i["id"]: i for i in issues}
    pending = {i["id"]: set(i["depends"]) for i in issues}
    for i, deps in pending.items():
        missing = deps - by_id.keys()
        if missing:
            sys.exit(f"{i} depende de ids inexistentes: {missing}")
    key = lambda i: int(i[1:])
    ready = [(key(i), i) for i, d in pending.items() if not d]
    heapq.heapify(ready)
    out = []
    while ready:
        _, i = heapq.heappop(ready)
        out.append(by_id[i])
        for j, d in pending.items():
            if i in d:
                d.discard(i)
                if not d and by_id[j] not in out and (key(j), j) not in ready:
                    heapq.heappush(ready, (key(j), j))
    if len(out) != len(issues):
        sys.exit("Hay un ciclo en las dependencias")
    return out


def gh(args, dry, capture=False):
    if dry:
        print("  [dry-run] gh", " ".join(a if " " not in a else repr(a) for a in args))
        return ""
    r = subprocess.run(["gh", *args], capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"Falló: gh {' '.join(args)}\n{r.stderr}")
    return r.stdout.strip()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True, help="usuario/repo")
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()

    issues = topo([parse(p) for p in sorted(ISSUES.glob("B*.md"))])
    state = json.loads(STATE.read_text()) if STATE.exists() else {}

    print("Etiquetas")
    for label in sorted({l for i in issues for l in i["labels"]}):
        color = next((c for p, c in LABEL_COLORS.items() if label.startswith(p)), "ededed")
        gh(["label", "create", label, "--repo", a.repo, "--color", color,
            "--description", LABEL_DESC.get(label, ""), "--force"], a.dry_run)

    print("Milestones")
    existing = set()
    if not a.dry_run:
        out = gh(["api", f"repos/{a.repo}/milestones?state=all", "--paginate", "--jq", ".[].title"], False)
        existing = set(out.splitlines())
    for title, desc in MILESTONES.items():
        if title not in existing:
            gh(["api", "-X", "POST", f"repos/{a.repo}/milestones",
                "-f", f"title={title}", "-f", f"description={desc}"], a.dry_run)

    print("Issues")
    for i in issues:
        if i["id"] in state:
            print(f"  {i['id']} ya existe como #{state[i['id']]}")
            continue
        body = re.sub(r"\bB(\d{2})\b",
                      lambda m: f"#{state[m.group(0)]}" if m.group(0) in state else m.group(0),
                      i["body"])
        with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False, encoding="utf-8") as f:
            f.write(body)
        args = ["issue", "create", "--repo", a.repo, "--title", i["title"], "--body-file", f.name,
                "--milestone", i["milestone"]]
        for label in i["labels"]:
            args += ["--label", label]
        url = gh(args, a.dry_run)
        if a.dry_run:
            state[i["id"]] = f"?{i['id']}"
            continue
        state[i["id"]] = int(url.rsplit("/", 1)[-1])
        STATE.write_text(json.dumps(state, indent=1))
        print(f"  {i['id']} → #{state[i['id']]} {i['title']}")
    print("Hecho.")


if __name__ == "__main__":
    main()
