-- 0008: qué decide compartir el corredor de cada resultado suyo (#36, docs/paquete.md).
--
-- Sin fila, vale lo que diga el ajuste `sharing.default_choice`. 'none' = no se comparte.
CREATE TABLE result_sharing (
    result_id INTEGER PRIMARY KEY REFERENCES results (id) ON DELETE CASCADE,
    choice    TEXT    NOT NULL CHECK (choice IN ('none', 'aggregates', 'legs', 'track'))
);
