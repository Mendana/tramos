# Compartir con la entrenadora

Todo se guarda en tu equipo. Tu entrenadora solo ve lo que tú compartes, carrera a carrera, y lo
ve en solo lectura ([Modo entrenadora](modo-entrenadora.md)).

## Cómo viaja: paquetes y carpeta compartida

No hay servidor. Cada carrera que compartes es un fichero (un **paquete**) que la app deja en una
**carpeta compartida**: una carpeta de Drive, OneDrive, Dropbox… que tienen tu app y la de tu
entrenadora. Tú la eliges en [Mi perfil](perfil.md); ella, en [Ajustes](ajustes.md).

- La app deja el paquete al importar la carrera y lo actualiza sola cada vez que cambias algo de
  lo que lleva (etiquetas, formato, desfase, qué se comparte).
- La app de la entrenadora mira la carpeta al abrirse y cada minuto, y recoge lo nuevo.
- Si un paquete no se puede escribir (la carpeta no está, no hay permiso), la carrera te lo dice y
  se vuelve a intentar.

## Qué se comparte de cada carrera

| Nivel                | Qué lleva                                                                                                                      |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| Nada                 | No se deja ningún fichero.                                                                                                     |
| Resumen              | Fecha, carrera, categoría, resultado, tiempo, tiempo perdido, errores y rendimiento. La entrenadora no puede abrir la carrera. |
| Tramos (por defecto) | Además, los splits de tu recorrido y tus etiquetas: la entrenadora ve la carrera entera, sin mapa.                             |
| Track completo       | Además, el track del reloj: GPS, pulso, altitud y cadencia. Con mapa.                                                          |

Se elige **por defecto** en [Mi perfil](perfil.md) y se cambia **en cada carrera**, en su
cabecera.

> Compartes tramos por defecto. En una carrera elegiste «track completo» porque querías que tu
> entrenadora viera tu ruta en el mapa. En otra, en la que el reloj grabó también el camino desde
> casa, eliges «resumen».

## Qué nunca sale

- El .spl original, que trae datos de todos los corredores (como fechas de nacimiento). Con
  «tramos» viaja solo una copia reducida de tu recorrido: nombre, apellidos y club de cada
  corredor (lo mismo que publican los resultados), sus picadas y su resultado. Sin dorsales,
  tarjetas ni sexo de nadie.
- Tu tarjeta SI. Tú vas con el nombre que escribiste en Mi perfil y un identificador al azar.

## Ojo con

- **Quien tenga acceso a la carpeta puede leer los paquetes**, y el servicio de sincronización los
  guarda en sus servidores.
- **Dejar de compartir** una carrera (pasarla a «nada») borra su fichero de la carpeta, pero lo que
  la entrenadora ya hubiera recibido se queda en su app.

Detalle técnico:
[paquete](https://github.com/Mendana/tramos/blob/main/docs/paquete.md) y
[datos y privacidad](https://github.com/Mendana/tramos/blob/main/docs/datos-y-privacidad.md).
