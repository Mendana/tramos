-- 0007: paquetes por carrera recibidos de los corredores (#35, docs/paquete.md).
--
-- Como mucho uno por (runner_id, race_id): reexportar sustituye al anterior. `content` es el
-- JSON del paquete tal cual llegó; las columnas de al lado son para buscar sin leerlo.
CREATE TABLE received_packages (
    id                   INTEGER PRIMARY KEY,
    runner_id            TEXT    NOT NULL,
    race_id              TEXT    NOT NULL,
    level                TEXT    NOT NULL CHECK (level IN ('aggregates', 'legs', 'track')),
    format_version       INTEGER NOT NULL CHECK (format_version >= 1),
    exported_at_epoch_ms INTEGER NOT NULL,
    imported_at_epoch_ms INTEGER NOT NULL,
    content              TEXT    NOT NULL,
    UNIQUE (runner_id, race_id)
);
