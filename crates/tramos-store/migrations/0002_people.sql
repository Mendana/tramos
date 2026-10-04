-- 0002: personas e identidad del corredor entre carreras. Ver docs/almacenamiento.md.
--
-- `runners` es por carrera (una fila por resultado). Una persona agrupa los resultados que el
-- usuario sabe que son de la misma persona, carrera tras carrera.

-- Personas. Solo guardan lo que escribe el usuario: sin fecha de nacimiento ni datos del .spl
-- (docs/datos-y-privacidad.md).
CREATE TABLE people (
    id                  INTEGER PRIMARY KEY,
    -- Nombre que se muestra en la app; no tiene por qué coincidir con el de ningún .spl.
    display_name        TEXT    NOT NULL CHECK (length(trim(display_name)) > 0),
    -- Texto libre del usuario.
    notes               TEXT,
    created_at_epoch_ms INTEGER NOT NULL
);

-- Un resultado pertenece como mucho a una persona. Borrar la persona desvincula sus
-- resultados (SET NULL), no los borra.
ALTER TABLE results ADD COLUMN person_id INTEGER REFERENCES people (id) ON DELETE SET NULL;
CREATE INDEX results_person ON results (person_id);
