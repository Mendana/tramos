# Fixtures

Datos de prueba **públicos**: solo anonimizados o sintéticos.

- Los datos reales van en `fixtures/private/`, que está en `.gitignore`.
- Para crear un fixture público a partir de uno real, usa las herramientas de anonimizado de
  `tools/` (issues de M0) y revisa el resultado antes de subirlo.
- Cada fixture lleva al lado un fichero `.expected.json` con la salida esperada, que usan los tests.

## Contenido

| Fichero | Qué es |
| --- | --- |
| `spl/baltanas-anon.spl` | Sprint de Baltanás (Liga Norte / Liga FOCYL), anonimizado: 18 categorías, 275 corredores, 9 recorridos. |
| `spl/baltanas-anon.expected.json` | Salida de `tools/reference/winsplits_spl.py` sobre el fichero anterior. |
| `fit/baltanas-sintetico.fit` | FIT **sintético** (posiciones inventadas) del corredor de M-SEN con tarjeta 143 (puesto 16) del fixture anterior. |
| `fit/baltanas-sintetico.truth.json` | Respuesta conocida del FIT anterior: balizas, tramos, rodeo y parada. |

Para regenerarlos desde el original (que solo existe en `fixtures/private/`):

```bash
python3 tools/anonimizar_spl.py fixtures/private/baltanas.spl fixtures/spl/baltanas-anon.spl
```

## FIT sintético

`tools/fit_sintetico.py` inventa coordenadas para las balizas del recorrido de un corredor del
.spl (zona urbana de Baltanás, tramos de 50 a 250 m en línea recta) y genera un track a 1 Hz que
pasa por cada baliza exactamente en su hora de picada. Las horas del .spl son locales
(Europe/Madrid): el 3 de octubre de 2026 es horario de verano, así que UTC = local − 2 h.

- Corredor: M-SEN, tarjeta SI 143 (= dorsal; el campo id 0x80 del .spl se repite entre
  corredores), clasificado 16.º, 20 balizas y 21 tramos. Salida 18:13:00 local (16:13:00Z),
  meta 18:38:40 (16:38:40Z).
- Track: de 60 s antes de la salida a 60 s después de la meta (1661 records,
  16:12:00Z–16:39:40Z), con posición, altitud, pulso, cadencia, distancia y velocidad.
  Cadencia en rpm de un pie, como los Garmin en carrera (pasos/min = 2 × cadencia).
  `fitparser` (y `fitdecode`) devuelven altitud y velocidad como `enhanced_altitude` y
  `enhanced_speed`.
- Rodeo: tramo 9 (42 → 46, split 427 s), 1215 m recorridos frente a 250 m en línea recta
  (ratio 4,86). El resto de tramos tiene ratio < 1,5.
- Parada: 30 s con velocidad 0 en el tramo 9, de 16:23:32Z a 16:24:02Z (192 s después de la
  picada en la 42). Fuera de ella, ningún segundo de carrera baja de 0,5 m/s.

Los valores exactos están en el `.truth.json`. Para regenerarlos (deterministas, semilla fija):

```bash
python3 tools/fit_sintetico.py fixtures/spl/baltanas-anon.spl fixtures/fit/baltanas-sintetico.fit --card 143
python3 -m unittest discover -s tools -p "test_*.py"
```

En Windows, `zoneinfo` necesita el paquete `tzdata` (`pip install tzdata`).
