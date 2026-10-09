# Formatos de carrera

No se corre igual un sprint que una larga, así que las [Estadísticas](estadisticas.md) separan las
carreras por formato:

| Formato | Cómo es                                                                         |
| ------- | ------------------------------------------------------------------------------- |
| Sprint  | Unos 15 minutos, normalmente urbano. Tramos cortos y muchas decisiones rápidas. |
| Media   | Unos 35 minutos, en monte técnico. Mucha lectura de detalle.                    |
| Larga   | De 50 a 60 minutos. Pesa la elección de ruta.                                   |

## De dónde sale

El .spl no dice el formato. Al [importar](importar.md), la app lo **sugiere** por lo que tardaron
los ganadores: toma el tiempo del ganador de cada categoría y mira la mediana. Por debajo de 25
minutos, sprint; por debajo de 45, media; si no, larga. Tú lo confirmas o lo cambias.

> Los ganadores de las categorías tardan 12, 13, 14, 15 y 50 minutos (esta última, una categoría
> con un solo corredor que fue con calma). La mediana es 14 minutos: sprint, aunque una categoría
> tardara 50.

## Cambiarlo

En la cabecera de la [carrera](carrera.md), el desplegable de formato. El cambio se guarda al
momento y mueve la carrera de grupo en las Estadísticas. Las carreras **sin formato** salen en su
propia fila: no se esconden, para que veas que falta asignárselo.

Detalle técnico:
[modelo, «Formato de carrera»](https://github.com/Mendana/tramos/blob/main/docs/modelo.md#formato-de-carrera).
