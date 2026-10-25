# Carreras de tus atletas

En el bloque [Atletas](atletas.md), si entrenas: **Carreras de atletas** junta a los atletas que corrieron la
misma carrera, para verlos uno al lado del otro. La app sabe que es la misma carrera porque todos
importaron el mismo .spl, aunque cada uno te la mande por su lado.

## La lista

Una fila por carrera que te ha llegado, de la más reciente a la más antigua: fecha, nombre,
formato y qué atletas la corrieron (cuántos y sus nombres). Las que corrieron **varios** van
destacadas, con el número en color. Si tú también la corriste y la tienes importada, cuentas como
uno más («Tu nombre (tú)»). Un clic abre la carrera.

## Una carrera

Arriba, una tabla con todos los que la corrieron: categoría, resultado (y su puesto en el
recorrido, juntando las categorías que lo comparten), tiempo, [tiempo perdido](tiempo-perdido.md)
con sus errores y qué te compartió: el resumen, los tramos o el track completo. Un clic en un
atleta abre su carrera, como la ve él. Si solo la ha compartido uno, la vista se abre igual, sin
comparación.

Cada atleta se mide contra **su propio** ritmo de ese día y con **sus** umbrales: los números son
los mismos que ve él en su carrera. No sale quién es más rápido, sino quién falló dónde.

Si corrieron **recorridos distintos** (otras categorías), un selector elige el recorrido: las
gráficas por tramo y el mapa son de uno. Las fichas con los nombres hacen de leyenda; si hay más
de cuatro atletas en el recorrido, eligen quién sale (cuatro como mucho, uno por color). Cada atleta guarda su color mientras estás
en la carrera: si quitas a uno, los demás no cambian, y el siguiente que añadas coge el que quedó
libre.

### Diferencia con el tiempo ideal

Una línea por atleta con lo que va por detrás del tiempo ideal del recorrido al acabar cada tramo,
como en [Frente al grupo](carrera-grupo.md): cuanto más abajo, más lejos del ideal. La que baja de
golpe es un tramo malo.

### Pérdida por tramo comparada

En cada tramo, una columna por atleta con los segundos perdidos (arriba) o ganados (abajo) frente a
lo esperado con el rendimiento habitual de cada uno: cada atleta, contra su propio ritmo.

### Tramos que fallaron varios

Los tramos en los que fallaron dos atletas o más, con lo que perdió cada uno y el
[tipo de error](tipos-de-error.md) que etiquetó, si lo hizo. Qué fue error lo dice su etiqueta y,
si no la tiene, el cálculo («sin revisar»). «Ver en el mapa» lleva el tramo al mapa.

> Tres de tus atletas corrieron la misma media. En «Tramos que fallaron varios» aparece el
> tramo 7: dos lo fallaron, uno etiquetado como «Salida de baliza» y otro sin revisar. En el
> mapa, con ese tramo elegido, ves que los dos salieron de la baliza hacia el mismo sitio
> equivocado.
> (Ejemplo inventado.)

### Elección de ruta

Si dos atletas o más salen en el mapa, los tramos en los que fueron por sitios distintos, con
cuántas **opciones** de ruta hubo y cuánto separó la mejor de la peor. Dos rutas son la misma
opción si van a menos de 30 m una de otra de media. Un clic en un tramo lo elige en el mapa y
enseña sus opciones, de la más rápida a la más lenta (por la mediana de sus tiempos), con la
distancia, la subida y quién fue por cada una. Pulsa una opción para resaltar sus rutas en el
mapa. Si alguien falló ese tramo sale «(con error)»: su tiempo dice más de su error que de la
ruta.

> En una larga, en «Elección de ruta» sale el tramo 5 con dos opciones y «+0:40». La opción A,
> por el camino, la cogieron dos atletas; la B, recta por el monte, uno, 40 s más lento.
> Resaltando la B ves que cruzó una vaguada que los otros rodearon.
> (Ejemplo inventado.)

### Mapa

El track de cada atleta que te compartió el track completo, de su color, y las balizas una sola
vez (en la media de donde estaba cada uno al picarlas). Un clic en un tramo deja solo ese tramo de
todos; otro clic lo quita. Solo salen los atletas elegidos en las fichas: si una opción es de
alguien que no sale, en el mapa no se ve.

Quien solo te compartió el resumen sale en la tabla, con un aviso, pero no en las gráficas ni en
el mapa; quien compartió los tramos sin el track, en las gráficas pero no en el mapa.

Desde la carrera de un atleta (o desde la tuya), **Ver con los demás atletas (3)** abre esta
misma vista, si otros atletas tuyos la corrieron.

Detalle técnico:
[app, «Atletas»](https://github.com/Mendana/tramos/blob/main/docs/app.md#atletas-37-119) y
[tiempo perdido, «Una carrera con varios atletas»](https://github.com/Mendana/tramos/blob/main/docs/tiempo-perdido.md#una-carrera-con-varios-atletas)
y [segmentación, «Elección de ruta»](https://github.com/Mendana/tramos/blob/main/docs/segmentacion.md#elección-de-ruta-190).
