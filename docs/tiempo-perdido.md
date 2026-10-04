# Tiempo perdido

Adaptación del método de WinSplits. Todo se calcula por **recorrido** (categorías con la misma
secuencia de balizas agrupadas), no por categoría.

## Definiciones

Para el tramo *i* de un recorrido:

- **Referencia** `ref_i`: media del 25 % más rápido de los splits válidos del tramo
  (redondeando hacia arriba, mínimo 1 corredor). Solo cuentan corredores clasificados
  (estado 0) con picada en ambos extremos del tramo.
- **Índice de rendimiento** `IR_i = ref_i / t_i`, donde `t_i` es el split del corredor.
  100 % = igual que la referencia; más alto = mejor.
- **Rendimiento habitual** del corredor en la carrera: mediana de sus `IR_i` ponderada por `ref_i`
  (WinSplits pondera por longitud; aquí no hay longitudes en el .spl).
- **Tiempo esperado** `esp_i = ref_i / habitual`.
- **Pérdida** `p_i = t_i − esp_i` (segundos, puede ser negativa) y `p_i / esp_i` (porcentaje).
- **Error**: el tramo es error si `p_i > umbral_segundos` **y** `p_i / esp_i > umbral_porcentaje`.
  Ambos umbrales son configurables. Valores iniciales: 15 s y 10 %.
- **Tiempo sin errores**: tiempo total menos la suma de pérdidas de los tramos con error.

## Exclusiones

- El último tramo (de la última baliza, normalmente la 100 o la 200, a meta) se calcula pero se
  excluye de los análisis de patrones.
- Tramos con referencia menor de 20 s: se excluyen de los análisis de patrones.
- Tramos sin picada en alguno de sus extremos: sin split, sin pérdida.

## Fiabilidad

- Con pocos corredores en el recorrido la referencia es frágil. Si el recorrido tiene menos de
  4 corredores válidos, se marca la carrera como "referencia débil" en la interfaz.
- La referencia de las preguntas históricas puede pasar a ser el propio histórico del corredor;
  eso queda fuera del MVP.

## Ejemplo de test

Recorrido con 4 corredores y 2 tramos; splits del tramo 1: 60, 62, 70, 90 s → `ref_1 = 60`
(25 % de 4 = 1 corredor). Un corredor con 90 s en el tramo 1 tiene `IR_1 = 0,667`.
Los tests del núcleo deben incluir casos así, calculados a mano.
