-- 0009: grupos de atletas de quien entrena (#120, docs/almacenamiento.md).
--
-- Los miembros apuntan al `runner_id` de los paquetes (docs/paquete.md), no a una persona ni a un
-- nombre: un atleta sigue en sus grupos aunque cambie su nombre visible, y quien entrena se
-- incluye con su propio identificador. Un atleta puede estar en varios grupos. Borrar un grupo
-- borra sus filas de miembros, nunca los paquetes recibidos.
CREATE TABLE athlete_groups (
    id                  INTEGER PRIMARY KEY,
    name                TEXT    NOT NULL CHECK (length(trim(name)) > 0),
    description         TEXT    NOT NULL DEFAULT '',
    -- `#rrggbb`.
    color               TEXT    NOT NULL,
    created_at_epoch_ms INTEGER NOT NULL
);

CREATE TABLE athlete_group_members (
    group_id  INTEGER NOT NULL REFERENCES athlete_groups (id) ON DELETE CASCADE,
    runner_id TEXT    NOT NULL,
    PRIMARY KEY (group_id, runner_id)
);
