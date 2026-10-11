# Qué hay de nuevo en Tramos

Lo que cambia en cada versión, explicado para quien usa la app. Este texto es el que ve el usuario
en el aviso de «hay una versión nueva» (botón «Qué trae») y en la descripción de la release.

Cómo se mantiene (`docs/distribucion.md`, «Publicar una versión»):

- Una sección por versión, con el título exacto `## X.Y.Z`, la más nueva arriba.
- Solo párrafos y listas con `- `, en español y en llano: lo que se ve o se puede hacer ahora, no
  detalles técnicos. Sin títulos dentro de la sección.
- Mientras no se publica, los cambios van en `## Sin publicar`. El PR que sube la versión cambia
  ese título por `## X.Y.Z` y deja arriba un `## Sin publicar` vacío. La release falla si no hay
  sección para el tag.

## Sin publicar

- Las carreras de tus atletas que corrieron lo mismo se ven juntas: una pantalla «Carreras de atletas» en el
  bloque Atletas, con el mapa del recorrido con los tracks de todos y los tramos que fallaron
  varios.
- En «Frente al grupo» se marcan tus atletas entre los rivales y salen los primeros.
- En las estadísticas de un grupo, los textos hablan del grupo («fallan», «media del grupo») en
  vez de «tus carreras».
- Si un atleta te manda otra vez una carrera que ya habías visto, ya no cuenta como carrera nueva
  en las novedades: sale como actualizada.
- El resumen en frases avisa cuando, a lo largo de tus carreras, tu pulso va más alto antes de
  los errores que antes de un tramo limpio.
- Nuevo apartado «Acerca de Tramos» en Ajustes, con la versión, el autor, la licencia y los
  créditos. Desde ahí puedes informar de un fallo por correo o copiar los datos para el informe.
- El tema claro u oscuro elegido se aplica desde que se abre la ventana, sin un destello del otro.

## 0.2.1

- Tramos tiene ya su propio icono: la baliza sobre el magenta, en el instalador, en la ventana y
  en la barra de tareas.

## 0.2.0

- Primera versión que se actualiza sola. Al arrancar, Tramos mira si hay una versión nueva y te
  avisa arriba; si quieres, la instala con un botón y se vuelve a abrir. Nada se descarga sin que
  lo pidas, y tus carreras y ajustes se quedan como están.
- En Ajustes, «Buscar actualizaciones» lo comprueba cuando quieras.
