-- 0003: deporte de la actividad del track (`Track::sport`). Ver docs/almacenamiento.md.
--
-- Los tracks guardados antes quedan sin deporte (NULL), como un FIT que no lo trae.
ALTER TABLE tracks ADD COLUMN sport TEXT;
