# Backlog

Una issue por fichero en `issues/`, con cabecera (título, milestone, etiquetas, dependencias) y
cuerpo (contexto, qué hacer, criterios de aceptación). Se suben a GitHub con:

```bash
gh auth login
python3 backlog/crear_issues.py --repo <usuario>/<repo> --dry-run   # revisar
python3 backlog/crear_issues.py --repo <usuario>/<repo>
```

El script crea etiquetas y milestones, sube las issues en orden de dependencias y convierte las
referencias `B08` en enlaces `#número`. Es reanudable gracias a `backlog/.creadas.json`.

Después de subirlas, GitHub es la fuente de verdad; estos ficheros quedan como histórico.
El orden recomendado está en `docs/trabajo-con-agentes.md`.
