# Atletas

Si entrenas a otros, en [Ajustes](ajustes.md) marca **Entreno a otros atletas**. Tus carreras
siguen igual: Inicio, Mis carreras, Estadísticas e Importar están siempre. Entrenar añade a la
barra lateral el bloque **Atletas**, para ver las carreras de quienes te las comparten **en solo
lectura**. Si además corres, usas las dos cosas a la vez, sin cambiar de modo.

## El bloque Atletas

- **Ver a**: un selector con los atletas que te han enviado algo y cuántas carreras. Al elegir
  uno, debajo salen sus **Carreras** y sus **Estadísticas**, exactamente como las ve él.
- **Comparar atletas**: todos juntos, o los de un grupo ([Comparar atletas](grupo.md)).
- **Grupos**: para organizarlos ([Grupos](grupos.md)).

Los números se recalculan en tu app con los umbrales de cada atleta, así que salen los mismos que
ve él. Las carreras que compartió solo con el resumen salen aparte y no se pueden abrir. El mapa
sale con los colores por cuantiles: las [zonas](zonas-del-mapa.md) de cada atleta no viajan con
sus carreras.

## Solo lectura

Mientras ves a un atleta, una franja arriba lo recuerda: «Estás viendo a… Solo lectura». No se
puede cambiar nada: el formato sale como etiqueta, el desfase del reloj sin botones y las
etiquetas de los tramos como texto. Sus datos solo cambian desde su propia app.

**Volver a lo mío**, en esa franja, te lleva a tu Inicio. También vuelven a lo tuyo Mis
carreras, Estadísticas e Importar del bloque «Lo mío».

## Cómo te llegan las carreras

Por una **carpeta madre** en Drive, OneDrive, Dropbox… con **una subcarpeta por atleta**. La
eliges en Ajustes. La app mira esa carpeta y todas las que tiene dentro (hasta tres niveles, sin
las ocultas ni los enlaces) al abrirse y cada minuto, y recoge los paquetes nuevos. No vuelve a
recoger tus propias carreras, si compartes, estén en la subcarpeta que estén. Al pie de la barra
lateral dice cuántos paquetes y de cuántos atletas ha recibido, y si alguno no se ha podido leer.

Si el mismo paquete (la misma carrera del mismo atleta) está en dos sitios, te quedas con el más
reciente.

> Creas en tu Drive una carpeta «Tramos grupo» y dentro una subcarpeta por atleta («Ana»,
> «Beto»…). Cada subcarpeta la compartes solo con su atleta, que la elige en Mi perfil y deja ahí
> sus carreras; así no ve las de los demás. Tú eliges la carpeta madre en Ajustes. Cuando uno
> etiqueta un error en su casa, te aparece, como mucho, un minuto después de que se sincronice la
> carpeta.

Más en [Compartir](compartir.md).

Detalle técnico: [app, «Atletas»](https://github.com/Mendana/tramos/blob/main/docs/app.md#atletas-37-119).
