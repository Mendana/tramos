# Modo entrenadora

La app tiene dos usos, que se eligen la primera vez que se abre y se cambian en
[Ajustes](ajustes.md):

- **Corredor**: importa sus carreras, las etiqueta y decide qué comparte.
- **Entrenadora**: recibe lo que comparten sus corredores y lo ve **en solo lectura**.

## Qué ve la entrenadora

- En la barra lateral, un **selector de corredor** con los que le han enviado algo y cuántas
  carreras. Debajo, sus **Carreras** y sus **Estadísticas**, exactamente como las ve él, y el
  [Grupo](grupo.md), con todos juntos.
- Los números se recalculan en su app con los umbrales de cada corredor, así que salen los mismos
  que ve él.
- Las carreras que el corredor compartió solo con el resumen salen aparte y no se pueden abrir.
- El mapa sale con los colores por cuantiles: las [zonas](zonas-del-mapa.md) de cada corredor no
  viajan con sus carreras.

## Solo lectura

La entrenadora no puede cambiar nada: no hay Importar ni Mi perfil, el formato sale como etiqueta,
el desfase del reloj sin botones y las etiquetas de los tramos como texto. Los datos de un
corredor solo cambian desde su propia app.

## Cómo le llegan las carreras

Por una carpeta compartida (Drive, OneDrive, Dropbox…) que ella elige en Ajustes y en la que
escriben sus corredores. La app mira esa carpeta (sin entrar en subcarpetas) al abrirse y cada
minuto. Al pie de la barra lateral dice cuántos paquetes y de cuántos corredores ha recibido, y si
alguno no se ha podido leer.

> La entrenadora crea en su Drive una carpeta «Tramos grupo», la comparte con sus corredores y la
> elige en Ajustes; cada corredor la elige en Mi perfil. Cuando uno etiqueta un error en su casa,
> a ella le aparece, como mucho, un minuto después de que se sincronice la carpeta.

Más en [Compartir con la entrenadora](compartir.md).
