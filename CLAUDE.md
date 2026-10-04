# Tramos — instrucciones para agentes

App de escritorio, de código abierto (MIT), para analizar carreras de orientación a pie:
cruza el FIT del reloj del corredor con los splits de WinSplits, trocea la carrera en tramos,
calcula el tiempo perdido y busca patrones de error a lo largo de muchas carreras.
Usuarios: un grupo de 5–10 corredores y su entrenadora (solo lectura).

Antes de tocar nada, lee la issue que te han asignado y los documentos de `docs/` que cite.

## Estructura

- `crates/tramos-core/`: núcleo en Rust. Modelos, importadores (.spl, FIT), alineación,
  segmentación, métricas y análisis. **Sin dependencias de interfaz ni de Tauri.**
- `crates/tramos-cli/`: CLI de desarrollo sobre el núcleo (`tramos analizar ...` → JSON).
- `app/`: app Tauri 2 + React + TypeScript (Vite). Solo llama al núcleo; no reimplementa cálculos.
- `docs/`: especificaciones. Son la fuente de verdad de formatos y algoritmos.
- `tools/reference/`: implementaciones de referencia en Python (oráculos para tests de paridad).
- `fixtures/`: datos de prueba públicos y anonimizados. `fixtures/private/` está en `.gitignore`.

## Comandos

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
# app (cuando exista)
cd app && npm ci && npm run build
npm run tauri dev
```

Una tarea no está terminada hasta que estos comandos pasan en local.

## Reglas

1. **Una issue por PR.** El PR incluye `Closes #N` y explica cómo se ha probado.
2. **Tests primero en el núcleo.** Todo algoritmo nuevo llega con tests que comprueban números
   calculados a mano o contra el oráculo de `tools/reference/`.
3. **Si cambias un algoritmo o formato, actualiza su documento en `docs/`** en el mismo PR.
4. **Nunca subas datos reales**: ni .spl sin anonimizar (traen nombres y fechas de nacimiento),
   ni FIT reales (GPS y pulso). Usa `fixtures/` o genera datos sintéticos.
5. **Licencias compatibles con MIT.** No añadas dependencias GPL, AGPL ni de uso no comercial.
6. **Código de librería sin `unwrap`/`expect`**; errores con `thiserror`. En la CLI, `anyhow`.
7. **Tiempos:** instantes absolutos en `chrono::DateTime<Utc>`; duraciones en segundos (`f64`).
   Las horas del .spl son hora local de la carrera (Europe/Madrid): conviértelas en la frontera.
8. **No amplíes el alcance.** Si descubres trabajo nuevo, menciónalo en el PR para abrir otra issue.
9. Idioma: identificadores y código en inglés; comentarios de dominio, docs, issues y PR en español.

## Glosario

- **Baliza / control**: punto que hay que visitar; tiene un código numérico.
- **Tramo / leg**: de una baliza a la siguiente. El primero sale del triángulo de salida.
- **Split**: tiempo de un tramo. **Picada**: instante en que el corredor marca la baliza.
- **Recorrido / course**: secuencia de balizas. Varias categorías pueden compartir recorrido.
- **Formato**: sprint (~15 min, urbano), media (~35 min, monte técnico), larga (50–60 min, elección de ruta).
- **Tiempo perdido**: ver `docs/tiempo-perdido.md`.
