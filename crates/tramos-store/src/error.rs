use chrono::{DateTime, Utc};

/// Errores del almacenamiento.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("error de SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error(
        "la base de datos tiene el esquema v{found}, más nuevo que el que entiende esta versión \
         de Tramos (v{supported})"
    )]
    SchemaTooNew { found: i64, supported: i64 },

    /// Los instantes se guardan en milisegundos; uno más fino no volvería igual.
    #[error("el instante {0} no se puede guardar: precisión mayor que el milisegundo")]
    SubMillisecondInstant(DateTime<Utc>),

    /// SQLite convierte NaN en NULL, así que no volvería igual.
    #[error("el campo {0} es NaN y no se puede guardar")]
    NotANumber(&'static str),

    #[error("no existe la carrera {0}")]
    EventNotFound(i64),

    #[error("no existe el resultado {0}")]
    ResultNotFound(i64),

    /// El resultado existe, pero no tiene track del reloj.
    #[error("el resultado {0} no tiene track del reloj")]
    TrackNotFound(i64),

    #[error("no existe el fichero original {0}")]
    SourceFileNotFound(i64),

    #[error("no existe la persona {0}")]
    PersonNotFound(i64),

    /// El nombre visible de una persona no puede estar vacío ni ser solo espacios.
    #[error("el nombre de la persona está vacío")]
    EmptyPersonName,

    /// Un resultado pertenece como mucho a una persona. Para cambiarlo de persona hay que
    /// desvincularlo antes (ver `docs/almacenamiento.md`).
    #[error("el resultado {result} ya está vinculado a la persona {person}")]
    ResultAlreadyLinked { result: i64, person: i64 },

    #[error("no existe el grupo {0}")]
    GroupNotFound(i64),

    /// El nombre de un grupo no puede estar vacío ni ser solo espacios.
    #[error("el grupo necesita un nombre")]
    EmptyGroupName,

    /// Los colores se guardan como `#rrggbb`.
    #[error("color no válido: {0:?} (tiene que ser #rrggbb)")]
    InvalidColor(String),

    /// Los tramos se numeran desde 1.
    #[error("tramo inválido: {0} (se numeran desde 1)")]
    InvalidLegIndex(usize),

    /// Un valor guardado no corresponde a ningún valor del modelo.
    #[error("dato inválido en la base de datos: {0}")]
    InvalidData(String),
}
