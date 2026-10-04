# Visión

Tramos es el cuaderno de errores del corredor de orientación, automatizado. Cada carrera se
convierte en filas de tramos que se acumulan, y las preguntas se hacen sobre ese histórico.
Las herramientas existentes (WinSplits, Livelox, Orion, OReplay) analizan una carrera cada vez;
Tramos responde a "¿en qué condiciones fallo yo, carrera tras carrera?".

## Decisiones

- App de escritorio local (Tauri 2), pensada para Windows. Sin servidor ni Docker.
- Solo orientación a pie, solo carreras individuales y sin entrenamientos por ahora.
- Entradas del MVP: FIT del reloj del corredor y .spl de WinSplits. OReplay, más adelante.
- Sin mapa de orientación en el MVP: cada baliza se sitúa con el GPS en el instante de su picada
  y la ruta se pinta sobre OpenStreetMap. La foto del mapa llega en una iteración posterior.
- El corredor etiqueta sus errores en tres niveles opcionales (`docs/taxonomia.md`).
- La entrenadora usa la misma app en modo solo lectura y recibe paquetes por carrera a través
  de una carpeta compartida.
- LLM en una fase posterior, con un cliente compatible con la API de OpenAI.
- Se empieza con los datos de un solo corredor; el grupo se suma cuando funcione.

## Entregas del MVP

| Tanda | Preguntas (ver `docs/preguntas.md`) | Necesita |
| --- | --- | --- |
| 1 | P1, P3, P4, P5, P6, P10 | Splits |
| 2 | P2, P7, P13 | Splits + FIT |
| 3 | P8, P9, P11, P14 | Etiquetas e histórico |
| 4 | P15 | Paquetes del grupo |
