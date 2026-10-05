# Preguntas del MVP y cómo se calculan

Definiciones de partida. Son revisables: cuando un análisis cambie, se actualiza aquí en el
mismo PR. Notación de `docs/tiempo-perdido.md`: `t_i` split, `ref_i` referencia, `IR_i` índice de
rendimiento, `esp_i` tiempo esperado, `p_i` pérdida. Todas las vistas muestran el número de casos
(n) en el que se apoyan.

## Tanda 1: solo splits

**P1. ¿Dónde pierdo tiempo?** Tabla por tramo: split, puesto en el tramo, `ref_i`, `IR_i`, `p_i` en
segundos y %, marca de error. Totales: tiempo perdido y tiempo sin errores. Umbrales configurables.

**P3. Carrera tramo a tramo.** Ruta sobre mapa base (P3 completa necesita FIT; sin FIT, solo
gráficas). Zona de gráficas desplegables: pérdida por baliza (barras) y pérdida acumulada (línea).
La pérdida acumulada tras el tramo *i* es la suma de `p_j` de los tramos con error hasta *i*: solo
sube en los errores y acaba en el tiempo perdido de la carrera (`docs/app.md`, "Gráficas").

**P4. Frente al grupo.** Mismas gráficas superponiendo compañeros elegidos del mismo recorrido.
Incluye la gráfica clásica de diferencia acumulada respecto al tiempo ideal (suma de las mejores
referencias de cada tramo). Usa solo splits públicos.

**P5. ¿Dónde gano y dónde pierdo?** Gráfica divergente a lo largo de la carrera: por tramo,
`g_i = esp_i − t_i` (positivo = mejor que tu rendimiento habitual) como barra sobre o bajo el eje,
más la línea acumulada. Se resaltan las rachas de dos o más tramos seguidos perdiendo.

**P6. ¿En qué formato rindo peor?** Por formato (sprint, media, larga): IR medio, tasa de error,
pérdida media por tramo y número de carreras. El formato se sugiere al importar (por la mediana de
los tiempos de los ganadores de las categorías, `docs/modelo.md`) y el corredor lo confirma.
Definiciones exactas del agregado (IR medio, tramos que cuentan, carreras sin formato) en
`docs/historico.md`.

**P10. ¿Soy consistente?** Desviación típica de `IR_i` en la carrera ponderada por `ref_i`
(menor = más consistente). Por carrera y como serie a lo largo del histórico.

## Tanda 2: splits + FIT

Las métricas de tramo del FIT (distancia, línea recta, velocidad en movimiento, paradas, subida
y bajada, pulso y cadencia) están definidas en `docs/metricas.md`.

**P2. ¿Lento o desorientado?** Versión inicial, heurística:
- `d_run` distancia recorrida en el tramo; `d_line` línea recta entre las posiciones GPS de las balizas.
- `r0`: mediana de `d_run / d_line` del corredor en sus tramos sin error de esa carrera.
- Desvío: `(d_run − r0 · d_line) / v_mov`, con `v_mov` la velocidad en movimiento del tramo.
- Paradas: tiempo con velocidad < 0,5 m/s, sin contar los 5 s siguientes a cada picada.
- Ritmo: el resto de `p_i`.
Resultado por tramo y agregado: qué parte de la pérdida es desvío, paradas o ritmo.

**P7. ¿Tramos largos o cortos?** Cubos por `ref_i` (no por el tiempo propio, para que un error no
mueva el tramo de cubo), en escala logarítmica: 20–30 s, 30–60 s, 1–2 min, 2–4 min, 4–8 min, más de
8 min. Por cubo: tasa de error, pérdida media en % y n. Excluye el último tramo y los de menos de 20 s.

**P13. ¿Me frena el desnivel?** Por tramo, subida y bajada acumuladas del FIT por cada 100 m
recorridos. La altitud se suaviza con una media móvil de ±5 s en el tiempo, que no cruza huecos
del track, y la subida es la suma de sus aumentos, sin umbral (`docs/metricas.md`, "Altitud
suavizada"). Clasificación inicial: subida si sube ≥ 4 m/100 m, bajada si baja
≥ 4 m/100 m, llano en otro caso (umbral configurable); si cumple las dos, la mayor. Por clase: IR
medio (ponderado por `ref_i`), tasa de error y n. Definiciones en `docs/historico.md`.

## Tanda 3: comportamiento y etiquetas

**P8. ¿Qué hago después de fallar?**
- Encadenamiento: tasa de error en el tramo siguiente a un error frente a la tasa tras un tramo limpio.
- Recuperación: en el tramo siguiente a un error, velocidad en movimiento frente a la mediana del
  corredor en tramos limpios; se marca si supera +5 % y si ese tramo acaba en error.
- Rachas limpias: tasa de error según los tramos limpios seguidos previos: 0, 1–2, 3–5, 6–10, más de 10.

**P9. ¿Qué errores son los más comunes?** Reparto por tipo y subtipo (`docs/taxonomia.md`),
cruzable con los cubos de P7 y con el formato. Se muestra qué parte de los errores no tiene tipo.
Lo marcado como físico no es un error de orientación: no entra en el reparto, pero se cuenta
aparte (`docs/historico.md`, "Errores más comunes (P9)").

**P11. ¿Entro peor en mapa tras días sin competir?** Días desde la carrera anterior importada,
en cubos: hasta 7, 8–14, 15–30, más de 30. Por cubo: IR medio de los tres primeros tramos y tasa de
error del primer tercio de la carrera.

**P14. ¿El cansancio anticipa el error?** Dato débil con pulso de muñeca; se muestra como tal.
- Deriva: relación pulso/velocidad por tercio de carrera en tramos limpios.
- Antes del error: pulso medio en el tramo previo a un error frente al de tramos previos a tramos
  limpios, en la misma fase de carrera.
- Esfuerzo percibido de las etiquetas, cuando exista.

## Tanda 4: entrenadora

**P15.** La entrenadora ve todo lo de cada corredor como si fuera él, en solo lectura, y una vista
de grupo: tabla de corredores por métricas (IR, tasa de error, tipos de error, P7 y P13) y
comparación de todos contra todos en las carreras compartidas.

## Descartadas por ahora

P12 (salida de baliza por ritmo), P16 (ficha por corredor) y la tendencia de temporada.
