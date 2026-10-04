-- 0001: esquema inicial. Ver docs/almacenamiento.md.
--
-- Convenciones:
-- - Instantes: INTEGER, milisegundos desde la época Unix en UTC (columnas *_epoch_ms).
-- - Fechas locales de carrera: TEXT 'AAAA-MM-DD'.
-- - Duraciones: REAL en segundos (columnas *_s).
-- - Orden de las listas del modelo: columna `position`, desde 0.
-- - Enumerados del modelo: TEXT en snake_case, como en el JSON de tramos_core::model.

-- Ficheros originales, con su contenido, para poder recalcular. El .spl incluye fechas de
-- nacimiento: esta tabla nunca sale de la base local (docs/datos-y-privacidad.md).
-- `sha256` es UNIQUE: importar dos veces el mismo fichero no duplica su contenido.
CREATE TABLE source_files (
    id                   INTEGER PRIMARY KEY,
    kind                 TEXT    NOT NULL CHECK (kind IN ('spl', 'fit')),
    -- Ruta de origen al importar; solo informativa.
    path                 TEXT    NOT NULL,
    sha256               TEXT    NOT NULL UNIQUE
        CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
    size_bytes           INTEGER NOT NULL,
    content              BLOB    NOT NULL,
    imported_at_epoch_ms INTEGER NOT NULL,
    CHECK (typeof(content) = 'blob' AND size_bytes = length(content))
);

-- Carreras.
CREATE TABLE events (
    id             INTEGER PRIMARY KEY,
    name           TEXT,
    date           TEXT    NOT NULL
        CHECK (date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    source_file_id INTEGER REFERENCES source_files (id) ON DELETE SET NULL
);
CREATE INDEX events_source_file ON events (source_file_id);

-- Recorridos: uno por secuencia de balizas distinta dentro de la carrera.
CREATE TABLE courses (
    id       INTEGER PRIMARY KEY,
    event_id INTEGER NOT NULL REFERENCES events (id) ON DELETE CASCADE
);
CREATE INDEX courses_event ON courses (event_id);

-- Balizas del recorrido en orden, sin salida ni meta.
CREATE TABLE course_controls (
    course_id INTEGER NOT NULL REFERENCES courses (id) ON DELETE CASCADE,
    position  INTEGER NOT NULL CHECK (position >= 0),
    code      INTEGER NOT NULL CHECK (code BETWEEN 0 AND 65535),
    PRIMARY KEY (course_id, position)
) WITHOUT ROWID;

-- Categorías. Varias pueden apuntar al mismo recorrido.
CREATE TABLE classes (
    id         INTEGER PRIMARY KEY,
    event_id   INTEGER NOT NULL REFERENCES events (id) ON DELETE CASCADE,
    position   INTEGER NOT NULL CHECK (position >= 0),
    source_id  INTEGER NOT NULL CHECK (source_id BETWEEN 0 AND 4294967295),
    name       TEXT    NOT NULL,
    short_name TEXT,
    course_id  INTEGER NOT NULL REFERENCES courses (id),
    UNIQUE (event_id, position)
);
CREATE INDEX classes_course ON classes (course_id);

-- Corredores tal y como aparecen en una carrera. Sin fecha de nacimiento, a propósito.
CREATE TABLE runners (
    id          INTEGER PRIMARY KEY,
    event_id    INTEGER NOT NULL REFERENCES events (id) ON DELETE CASCADE,
    source_id   INTEGER NOT NULL CHECK (source_id BETWEEN 0 AND 4294967295),
    given_name  TEXT    NOT NULL,
    family_name TEXT    NOT NULL,
    club        TEXT,
    bib         INTEGER CHECK (bib BETWEEN 0 AND 4294967295),
    si_card     INTEGER CHECK (si_card BETWEEN 0 AND 4294967295),
    sex         TEXT    CHECK (sex IN ('male', 'female'))
);
CREATE INDEX runners_event ON runners (event_id);

-- Resultado de un corredor en una categoría.
CREATE TABLE results (
    id          INTEGER PRIMARY KEY,
    class_id    INTEGER NOT NULL REFERENCES classes (id) ON DELETE CASCADE,
    position    INTEGER NOT NULL CHECK (position >= 0),
    runner_id   INTEGER NOT NULL UNIQUE REFERENCES runners (id) ON DELETE CASCADE,
    status      TEXT    NOT NULL
        CHECK (status IN ('ok', 'not_classified', 'did_not_start', 'unknown')),
    -- Código original, solo para `unknown`.
    status_code INTEGER CHECK (status_code BETWEEN 0 AND 255),
    place       INTEGER CHECK (place BETWEEN 0 AND 65535),
    CHECK ((status = 'unknown') = (status_code IS NOT NULL)),
    UNIQUE (class_id, position)
);

-- Picadas en orden, de la salida a la meta. Sin hora: time_epoch_ms NULL.
CREATE TABLE punches (
    result_id     INTEGER NOT NULL REFERENCES results (id) ON DELETE CASCADE,
    position      INTEGER NOT NULL CHECK (position >= 0),
    code          INTEGER NOT NULL CHECK (code BETWEEN 0 AND 65535),
    time_epoch_ms INTEGER,
    PRIMARY KEY (result_id, position)
) WITHOUT ROWID;

-- Track del reloj: como mucho uno por resultado.
CREATE TABLE tracks (
    id             INTEGER PRIMARY KEY,
    result_id      INTEGER NOT NULL UNIQUE REFERENCES results (id) ON DELETE CASCADE,
    source_file_id INTEGER REFERENCES source_files (id) ON DELETE SET NULL
);
CREATE INDEX tracks_source_file ON tracks (source_file_id);

CREATE TABLE track_points (
    track_id       INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    position       INTEGER NOT NULL CHECK (position >= 0),
    time_epoch_ms  INTEGER NOT NULL,
    lat            REAL    NOT NULL,
    lon            REAL    NOT NULL,
    altitude_m     REAL,
    heart_rate_bpm INTEGER CHECK (heart_rate_bpm BETWEEN 0 AND 255),
    cadence_spm    REAL,
    distance_m     REAL,
    PRIMARY KEY (track_id, position)
) WITHOUT ROWID;

-- Tramos de un resultado con lo que calcula el análisis (docs/tiempo-perdido.md).
-- Se recalculan enteros cuando cambia `algorithm_version`.
CREATE TABLE legs (
    id                INTEGER PRIMARY KEY,
    result_id         INTEGER NOT NULL REFERENCES results (id) ON DELETE CASCADE,
    leg_index         INTEGER NOT NULL CHECK (leg_index >= 1),
    from_code         INTEGER NOT NULL CHECK (from_code BETWEEN 0 AND 65535),
    to_code           INTEGER NOT NULL CHECK (to_code BETWEEN 0 AND 65535),
    split_s           REAL,
    reference_s       REAL,
    -- ref / split, en fracción (1.0 = igual que la referencia).
    performance_index REAL,
    expected_s        REAL,
    loss_s            REAL,
    -- loss_s / expected_s, en fracción (0.10 = 10 %).
    loss_ratio        REAL,
    is_error          INTEGER CHECK (is_error IN (0, 1)),
    algorithm_version TEXT    NOT NULL,
    UNIQUE (result_id, leg_index)
);

-- Etiquetas del corredor (docs/taxonomia.md). Ningún nivel es obligatorio.
-- Apuntan a (result_id, leg_index) y no a legs.id para sobrevivir a un recálculo.
CREATE TABLE tags (
    id                  INTEGER PRIMARY KEY,
    result_id           INTEGER NOT NULL REFERENCES results (id) ON DELETE CASCADE,
    leg_index           INTEGER NOT NULL CHECK (leg_index >= 1),
    taxonomy_version    TEXT    NOT NULL,
    -- Nivel 1: ¿hubo error?
    confirmation        TEXT    CHECK (confirmation IN ('error', 'no_error', 'physical')),
    -- Nivel 2: claves del fichero de taxonomía.
    error_type          TEXT,
    error_subtype       TEXT,
    -- Nivel 3: contexto (las causas van en tag_causes).
    leg_part            TEXT    CHECK (leg_part IN ('start', 'middle', 'attack')),
    perceived_loss_s    REAL    CHECK (perceived_loss_s >= 0),
    effort              INTEGER CHECK (effort BETWEEN 1 AND 10),
    note                TEXT,
    created_at_epoch_ms INTEGER NOT NULL,
    updated_at_epoch_ms INTEGER NOT NULL,
    CHECK (error_subtype IS NULL OR error_type IS NOT NULL),
    UNIQUE (result_id, leg_index)
);

-- Causas percibidas (nivel 3, varias por etiqueta): claves del fichero de taxonomía.
CREATE TABLE tag_causes (
    tag_id INTEGER NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    cause  TEXT    NOT NULL,
    PRIMARY KEY (tag_id, cause)
) WITHOUT ROWID;

-- Ajustes clave-valor (umbrales, preferencias…).
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) WITHOUT ROWID;
