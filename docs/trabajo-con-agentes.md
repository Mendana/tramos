# Cómo trabajamos con agentes

## Principio

Cada issue debe poder cerrarse con una comprobación objetiva: tests en verde o una salida que se
compara con un oráculo. Por eso el núcleo y la CLI van antes que la interfaz, y las
especificaciones viven en `docs/`.

## Ciclo de una issue

1. La issue tiene contexto, tareas, criterios de aceptación y dependencias. Si le falta algo,
   se completa antes de dársela a un agente.
2. Un agente la implementa en su propia rama y abre un PR con `Closes #N`.
3. La CI pasa (formato, clippy, tests en Linux y Windows).
4. Revisión humana: obligatoria en `area:core`, ligera en `area:ui` y `area:ci`.
5. Merge. Si el agente detectó trabajo nuevo, se abre otra issue.

## Etiquetas

- `area:core`, `area:cli`, `area:app`, `area:ui`, `area:ci`, `area:docs`, `area:tools`
- `listo-para-agente`: está bien especificada. Antes de lanzarla, comprueba que las issues de
  "Depende de" están cerradas.
- `necesita-humano`: requiere datos reales, una decisión o probar en Windows.
- `tanda-1` … `tanda-4`: entrega del MVP a la que pertenece.

## Formas de lanzar agentes

- **En local** con Claude Code, una sesión por issue y cada una en su worktree, para trabajar
  varias en paralelo sin pisarse.
- **En la nube** con `claude --cloud "Resuelve la issue #N siguiendo CLAUDE.md"`: cada orden crea
  una sesión independiente que sigue aunque cierres el portátil; luego se crea el PR desde
  claude.ai/code.
- **Desde GitHub** con la Claude Code GitHub Action: comentar `@claude implementa esta issue` en la
  issue hace que Claude trabaje en el repositorio y abra el PR. Se instala con `/install-github-app`
  desde Claude Code y puede autenticarse con la suscripción (`CLAUDE_CODE_OAUTH_TOKEN`).

## Paralelismo

- Como máximo 3 agentes a la vez, en issues sin dependencias entre sí y que no toquen los mismos
  ficheros.
- El núcleo (algoritmos) se trabaja con la persona mirando; la interfaz y la CI se delegan más.

## Orden: olas

Cada ola solo depende de las anteriores; dentro de una ola, las issues pueden ir en paralelo.
Los ids son los de `backlog/issues/` (en GitHub, cada issue enlaza sus dependencias reales).

| Ola | Issues |
| --- | --- |
| 1 | B01 workspace · B03 anonimizador de .spl (necesita tu .spl real) |
| 2 | B02 CI · B04 FIT sintético · B05 modelo de dominio |
| 3 | B06 lector .spl · B07 lector FIT · B15 esqueleto de la app · B16 SQLite |
| 4 | B08 recorridos · B09 identificar corredor · B10 alineación · B17 ajustes · B35 paquete · B39 instalador |
| 5 | B11 segmentación · B12 tiempo perdido · B40 actualizaciones |
| 6–7 | B13 métricas FIT → B14 CLI |
| 8–9 | B18 importar → B19 tabla de tramos · B36 carpeta compartida · B41 lotes |
| 10 | B20 mapa · B21 gráficas · B30 etiquetado · B37 modo entrenadora |
| 11 | B22 P1/P3 · B24 P5 · B25 P6 · B27 P2 · B34 P14 |
| 12 | B23 P4 · B26 P10 · B28 P7 · B29 P13 · B32 P8 · B33 P11 |
| 13–14 | B31 P9 → B38 vista de grupo P15 |

Hito útil temprano: al cerrar la ola 7 ya puedes analizar tus carreras desde la terminal con
`tramos analizar`, antes de que exista la interfaz.
