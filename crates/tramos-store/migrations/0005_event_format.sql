-- 0005: formato de la carrera (sprint, media o larga), que confirma el corredor al importar.
-- Ver docs/almacenamiento.md y docs/modelo.md ("Formato de carrera").
--
-- Las carreras guardadas antes quedan sin formato (NULL).
ALTER TABLE events ADD COLUMN format TEXT CHECK (format IN ('sprint', 'middle', 'long'));
