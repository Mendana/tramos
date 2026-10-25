//! Exportar los datos para analizarlos fuera de la app (#217): una fila por carrera
//! (`races.csv`) y una por tramo (`legs.csv`), con un `schema.json` y un `LEEME.md` que cuentan
//! cada columna. Si se pide, también la serie de cada carrera con track (`samples.csv`, #219). El
//! formato completo está en `docs/exportar-datos.md`.
//!
//! Aquí no se calcula nada: las filas se arman con lo que ya dan [`crate::runner_report`], las
//! métricas de tramo ([`crate::metrics`]), el rendimiento físico ([`crate::physical`]) y la serie
//! de la carrera ([`crate::physical_series`]). Tampoco se lee la base ni se escriben ficheros:
//! eso lo hace la app, con estas filas y estos escritores.
//!
//! Privacidad: `races.csv` y `legs.csv` no llevan nombres, notas libres, coordenadas ni
//! resultados de otros corredores (solo la referencia del tramo, que ya se usa en toda la app).
//! Las coordenadas solo salen en `samples.csv` y solo si se piden (`lat` y `lon`).

use std::io::{self, Write};

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

use crate::event_kind::EventKind;
use crate::history::race_third;
use crate::metrics::{LegMetrics, TrackMetrics};
use crate::model::RaceStatus;
use crate::physical::{RacePhysical, SpeedKind};
use crate::physical_series::{RaceSeries, SeriesState};
use crate::race_format::RaceFormat;
use crate::runner_report::{LegReport, RunnerReport};
use crate::taxonomy::LegTag;

/// Versión del formato. Sube cuando se quita o cambia una columna; añadir columnas al final no
/// la cambia.
pub const FORMAT_VERSION: u32 = 1;

/// Segundos entre un instante y el siguiente en `samples.csv`. Un segundo es lo que piden los
/// modelos de secuencias, y cuesta unas 3.600 filas por hora de carrera (la pestaña Físico usa
/// 5 s para dibujar).
pub const SAMPLES_STEP_S: f64 = 1.0;

/// Qué ficheros opcionales se exportan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExportOptions {
    /// Incluir `samples.csv`, la serie de cada carrera con track.
    pub samples: bool,
    /// Añadir `lat` y `lon` a `samples.csv`. Sin `samples` no hace nada.
    pub coordinates: bool,
}

impl ExportOptions {
    /// Si `samples.csv` lleva las coordenadas.
    pub fn with_coordinates(self) -> bool {
        self.samples && self.coordinates
    }
}

/// Errores al escribir las tablas.
#[derive(Debug, Error)]
pub enum ExportError {
    #[error("no se pudo escribir: {0}")]
    Io(#[from] io::Error),
    #[error("una fila tiene {found} valores y la tabla {expected} columnas")]
    RowLength { expected: usize, found: usize },
}

/// Tipo de una columna.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ColumnType {
    Integer,
    Number,
    String,
    Boolean,
    /// `AAAA-MM-DD`.
    Date,
}

/// Una columna de una tabla: lo que se escribe en `schema.json`.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Column {
    pub name: &'static str,
    #[serde(rename = "type")]
    pub kind: ColumnType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
    pub description: &'static str,
    /// Documento de `docs/` donde está la definición.
    pub source: &'static str,
    /// Valores posibles, si es una categoría.
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pub allowed_values: &'static [&'static str],
}

const fn col(
    name: &'static str,
    kind: ColumnType,
    unit: Option<&'static str>,
    description: &'static str,
    source: &'static str,
) -> Column {
    Column {
        name,
        kind,
        unit,
        description,
        source,
        allowed_values: &[],
    }
}

const fn category(
    name: &'static str,
    description: &'static str,
    source: &'static str,
    allowed_values: &'static [&'static str],
) -> Column {
    Column {
        name,
        kind: ColumnType::String,
        unit: None,
        description,
        source,
        allowed_values,
    }
}

use ColumnType::{Boolean, Date, Integer, Number};

const LOST: &str = "docs/tiempo-perdido.md";
const METRICS: &str = "docs/metricas.md";
const PHYSICAL: &str = "docs/rendimiento-fisico.md";
const HISTORY: &str = "docs/historico.md";
const TAXONOMY: &str = "docs/taxonomia.md";
const EXPORT: &str = "docs/exportar-datos.md";

/// Estados de una carrera exportada (los no presentados no se exportan).
const STATUSES: &[&str] = &["ok", "not_classified", "unknown"];
/// Con qué velocidad se comparan los trozos de traslado (`docs/rendimiento-fisico.md`).
const SPEED_KINDS: &[&str] = &["flat", "moving"];

/// Columnas de `races.csv`, en orden.
pub const RACE_COLUMNS: &[Column] = &[
    col(
        "race_id",
        Integer,
        None,
        "Identificador estable de la carrera propia: el del resultado en la base de datos de la app. No cambia al recalcular ni al renombrar. Es la clave que une con legs.csv.",
        EXPORT,
    ),
    col(
        "event_id",
        Integer,
        None,
        "Identificador de la prueba en la base de datos: dos carreras propias con el mismo event_id son de la misma prueba.",
        EXPORT,
    ),
    col(
        "date",
        Date,
        None,
        "Día de la carrera (fecha local).",
        EXPORT,
    ),
    category(
        "kind",
        "Competición (race) o entrenamiento (training).",
        "docs/historico.md",
        &["race", "training"],
    ),
    category(
        "format",
        "Formato: sprint, middle o long. Vacío si no se ha fijado.",
        HISTORY,
        &["sprint", "middle", "long"],
    ),
    category(
        "status",
        "Estado en la clasificación. Los no presentados no se exportan.",
        EXPORT,
        STATUSES,
    ),
    col(
        "legs",
        Integer,
        None,
        "Tramos del recorrido, contando el último (el de la última baliza a meta).",
        LOST,
    ),
    col(
        "total_s",
        Number,
        Some("s"),
        "Tiempo de salida a meta. Vacío sin tiempo.",
        LOST,
    ),
    col(
        "place",
        Integer,
        None,
        "Puesto en la categoría. Solo los clasificados lo tienen.",
        EXPORT,
    ),
    col(
        "classified",
        Integer,
        None,
        "Clasificados de la categoría con tiempo, el corredor incluido: el «de 18» del puesto.",
        LOST,
    ),
    col(
        "course_runners",
        Integer,
        None,
        "Corredores con tiempo válido en el recorrido (todas las categorías que lo comparten): con ellos se calcula la referencia de cada tramo.",
        LOST,
    ),
    col(
        "lost_time_s",
        Number,
        Some("s"),
        "Tiempo perdido en los tramos con error (errores revisados).",
        LOST,
    ),
    col(
        "error_count",
        Integer,
        None,
        "Tramos con error (errores revisados: manda la etiqueta del corredor).",
        LOST,
    ),
    col(
        "time_without_errors_s",
        Number,
        Some("s"),
        "Tiempo total menos el tiempo perdido en los errores.",
        LOST,
    ),
    col(
        "place_without_errors",
        Integer,
        None,
        "Puesto que habría tenido en su categoría con su tiempo sin errores.",
        LOST,
    ),
    col(
        "usual_performance",
        Number,
        Some("ratio"),
        "Rendimiento habitual (IR medio de la carrera): mediana de los IR de los tramos ponderada por la referencia. 1 = 100 %, igual que la referencia; más es mejor.",
        LOST,
    ),
    col(
        "consistency",
        Number,
        Some("ratio"),
        "Consistencia (P10): desviación típica de los IR de los tramos que cuentan. 1 = 100 puntos de IR; menos es más regular. Vacío con menos de 2 tramos que cuenten.",
        LOST,
    ),
    col(
        "days_since_previous_race",
        Integer,
        Some("días"),
        "Días desde la carrera anterior en la que el corredor tomó la salida, sea cual sea su tipo y formato. Vacío en la primera.",
        HISTORY,
    ),
    col(
        "has_track",
        Boolean,
        None,
        "La carrera tiene el track del reloj (FIT) guardado y se ha podido alinear. Sin él, las columnas físicas van vacías.",
        METRICS,
    ),
    col(
        "track_duration_s",
        Number,
        Some("s"),
        "Suma de las duraciones de los tramos con track.",
        PHYSICAL,
    ),
    col(
        "track_distance_m",
        Number,
        Some("m"),
        "Distancia recorrida en los tramos con track.",
        PHYSICAL,
    ),
    col(
        "ascent_m",
        Number,
        Some("m"),
        "Subida acumulada con la altitud suavizada. Vacío sin altitud.",
        PHYSICAL,
    ),
    col(
        "descent_m",
        Number,
        Some("m"),
        "Bajada acumulada (positiva). Vacío sin altitud.",
        PHYSICAL,
    ),
    col(
        "moving_s",
        Number,
        Some("s"),
        "Tiempo en movimiento.",
        PHYSICAL,
    ),
    col(
        "stopped_s",
        Number,
        Some("s"),
        "Tiempo parado, sin los primeros segundos de cada tramo.",
        PHYSICAL,
    ),
    col(
        "gap_s",
        Number,
        Some("s"),
        "Tiempo en huecos del track.",
        PHYSICAL,
    ),
    col(
        "attack_s",
        Number,
        Some("s"),
        "Tiempo en las zonas de ataque de las balizas.",
        PHYSICAL,
    ),
    category(
        "speed_kind",
        "Velocidad con la que se comparan el traslado y sus mitades: flat (equivalente en llano) si el track tiene altitud, moving (real) si no.",
        PHYSICAL,
        SPEED_KINDS,
    ),
    col(
        "transit_s",
        Number,
        Some("s"),
        "Tiempo de traslado: en movimiento y fuera de las zonas de ataque.",
        PHYSICAL,
    ),
    col(
        "transit_distance_m",
        Number,
        Some("m"),
        "Distancia de traslado.",
        PHYSICAL,
    ),
    col(
        "transit_speed_mps",
        Number,
        Some("m/s"),
        "Velocidad real en el traslado.",
        PHYSICAL,
    ),
    col(
        "transit_flat_speed_mps",
        Number,
        Some("m/s"),
        "Velocidad equivalente en llano en el traslado. Vacío sin altitud.",
        PHYSICAL,
    ),
    col(
        "transit_heart_rate_bpm",
        Number,
        Some("ppm"),
        "Pulso medio en el traslado. Vacío con pulso en menos de la mitad del tiempo.",
        PHYSICAL,
    ),
    col(
        "efficiency_m_per_beat",
        Number,
        Some("m/latido"),
        "Eficiencia: metros por latido en el traslado, con la velocidad de speed_kind. Sube si va más rápido con el mismo pulso.",
        PHYSICAL,
    ),
    col(
        "cadence_spm",
        Number,
        Some("pasos/min"),
        "Cadencia media en el traslado.",
        PHYSICAL,
    ),
    col(
        "stride_m",
        Number,
        Some("m"),
        "Zancada: metros reales por paso en el traslado.",
        PHYSICAL,
    ),
    col(
        "heart_rate_bpm",
        Number,
        Some("ppm"),
        "Pulso medio de la salida a la meta, parado incluido.",
        PHYSICAL,
    ),
    col(
        "heart_rate_max_bpm",
        Number,
        Some("ppm"),
        "Pulso máximo con una media móvil de ±5 s.",
        PHYSICAL,
    ),
    col(
        "decoupling_pct",
        Number,
        Some("%"),
        "Desacople: caída de la eficiencia de la primera mitad del traslado a la segunda. Positivo: la segunda mitad cuesta más pulso por metro. Vacío si alguna mitad no llega a 2 min.",
        PHYSICAL,
    ),
    col(
        "speed_change_pct",
        Number,
        Some("%"),
        "Cambio de la velocidad (speed_kind) de la primera mitad a la segunda.",
        PHYSICAL,
    ),
    col(
        "heart_rate_change_bpm",
        Number,
        Some("ppm"),
        "Pulso de la segunda mitad menos el de la primera.",
        PHYSICAL,
    ),
    col(
        "stride_change_pct",
        Number,
        Some("%"),
        "Cambio de la zancada de la primera mitad a la segunda.",
        PHYSICAL,
    ),
    col(
        "vam_m_per_h",
        Number,
        Some("m/h"),
        "VAM: metros subidos por hora en los trozos de traslado con un 5 % de pendiente o más. Vacío con menos de 1 min de subida.",
        PHYSICAL,
    ),
    col(
        "climb_s",
        Number,
        Some("s"),
        "Tiempo de traslado en esos trozos de subida.",
        PHYSICAL,
    ),
    col(
        "heart_rate_load",
        Number,
        None,
        "Carga: minutos en cada zona de pulso de Ajustes por su número (1, 2, 3…), sumados. Solo se puede comparar entre carreras con las mismas zonas. Vacío sin zonas definidas o sin pulso.",
        PHYSICAL,
    ),
];

/// Columnas de `legs.csv`, en orden.
pub const LEG_COLUMNS: &[Column] = &[
    col(
        "race_id",
        Integer,
        None,
        "Carrera del tramo: el race_id de races.csv.",
        EXPORT,
    ),
    col(
        "leg",
        Integer,
        None,
        "Número del tramo, desde 1 (el que sale del triángulo de salida).",
        LOST,
    ),
    col(
        "legs",
        Integer,
        None,
        "Tramos del recorrido, contando el último.",
        LOST,
    ),
    col(
        "position",
        Number,
        Some("ratio"),
        "Posición relativa en la carrera: leg / legs. El último tramo vale 1.",
        HISTORY,
    ),
    col(
        "third",
        Integer,
        None,
        "Tercio de la carrera por número de tramos: 1, 2 o 3.",
        HISTORY,
    ),
    col(
        "is_last",
        Boolean,
        None,
        "Es el último tramo (de la última baliza a meta).",
        LOST,
    ),
    col(
        "excluded_from_patterns",
        Boolean,
        None,
        "No cuenta en los análisis de patrones: el último tramo, los de referencia corta (menos de 20 s) y los sin split.",
        LOST,
    ),
    col(
        "split_s",
        Number,
        Some("s"),
        "Tiempo del tramo. Vacío si falta alguna picada.",
        LOST,
    ),
    col(
        "reference_s",
        Number,
        Some("s"),
        "Referencia del tramo: media del 25 % más rápido de los splits válidos del recorrido. Es un agregado, no el resultado de nadie.",
        LOST,
    ),
    col(
        "reference_count",
        Integer,
        None,
        "Corredores en los que se basa la referencia.",
        LOST,
    ),
    col(
        "performance_index",
        Number,
        Some("ratio"),
        "IR del tramo: referencia / split. 1 = igual que la referencia; más es mejor.",
        LOST,
    ),
    col(
        "expected_s",
        Number,
        Some("s"),
        "Tiempo esperado: referencia / rendimiento habitual de la carrera.",
        LOST,
    ),
    col(
        "loss_s",
        Number,
        Some("s"),
        "Pérdida: split menos esperado. Puede ser negativa.",
        LOST,
    ),
    col(
        "loss_pct",
        Number,
        Some("%"),
        "Pérdida entre el tiempo esperado, en tanto por ciento.",
        LOST,
    ),
    col(
        "is_error",
        Boolean,
        None,
        "Error revisado: lo que dice la etiqueta del corredor y, sin ella, el cálculo. Es el que cuenta en toda la app.",
        LOST,
    ),
    col(
        "calculated_error",
        Boolean,
        None,
        "Error propuesto por el cálculo (los umbrales de Ajustes), diga lo que diga la etiqueta.",
        LOST,
    ),
    category(
        "tag_confirmation",
        "Respuesta del corredor en la etiqueta: error (sí), no_error (no) o physical (se perdió tiempo por el físico). Vacío sin respuesta.",
        TAXONOMY,
        &["error", "no_error", "physical"],
    ),
    col(
        "error_type",
        ColumnType::String,
        None,
        "Tipo de error de la etiqueta (clave de la taxonomía). Vacío sin tipo.",
        TAXONOMY,
    ),
    col(
        "error_subtype",
        ColumnType::String,
        None,
        "Subtipo de error de la etiqueta (clave de la taxonomía).",
        TAXONOMY,
    ),
    col(
        "effort",
        Integer,
        Some("1-10"),
        "Esfuerzo percibido de la etiqueta, de 1 a 10.",
        TAXONOMY,
    ),
    col(
        "distance_m",
        Number,
        Some("m"),
        "FIT: distancia recorrida en el tramo. Vacío sin track del tramo.",
        METRICS,
    ),
    col(
        "straight_m",
        Number,
        Some("m"),
        "FIT: línea recta entre las dos balizas.",
        METRICS,
    ),
    col(
        "distance_ratio",
        Number,
        Some("ratio"),
        "FIT: distancia / línea recta. Vacío si la línea recta mide menos de 1 m.",
        METRICS,
    ),
    col(
        "moving_speed_mps",
        Number,
        Some("m/s"),
        "FIT: velocidad en movimiento.",
        METRICS,
    ),
    col(
        "flat_speed_mps",
        Number,
        Some("m/s"),
        "FIT: velocidad equivalente en llano. Vacío sin altitud.",
        METRICS,
    ),
    col(
        "moving_s",
        Number,
        Some("s"),
        "FIT: tiempo en movimiento.",
        METRICS,
    ),
    col(
        "stopped_s",
        Number,
        Some("s"),
        "FIT: tiempo parado.",
        METRICS,
    ),
    col(
        "ascent_m",
        Number,
        Some("m"),
        "FIT: subida acumulada. Vacío sin altitud.",
        METRICS,
    ),
    col(
        "descent_m",
        Number,
        Some("m"),
        "FIT: bajada acumulada (positiva). Vacío sin altitud.",
        METRICS,
    ),
    col(
        "heart_rate_bpm",
        Number,
        Some("ppm"),
        "FIT: pulso medio. Vacío sin pulso.",
        METRICS,
    ),
    col(
        "cadence_spm",
        Number,
        Some("pasos/min"),
        "FIT: cadencia media.",
        METRICS,
    ),
    col(
        "attack_s",
        Number,
        Some("s"),
        "FIT: tiempo de ataque, de la entrada en la zona de la baliza a la picada.",
        METRICS,
    ),
    col(
        "days_since_previous_race",
        Integer,
        Some("días"),
        "Contexto: días desde la carrera anterior (el mismo valor que en races.csv).",
        HISTORY,
    ),
    col(
        "previous_leg_error",
        Boolean,
        None,
        "Contexto: el tramo anterior fue error revisado. Vacío en el primer tramo o si el anterior no tiene pérdida.",
        "docs/historico.md",
    ),
];

/// Estados de un instante de `samples.csv` (los de [`SeriesState`]).
const SAMPLE_STATES: &[&str] = &["transit", "attack", "stopped", "gap"];

/// Columnas de `samples.csv`, en orden: las dos últimas (`lat` y `lon`) solo salen si se piden
/// las coordenadas.
pub const SAMPLE_COLUMNS: &[Column] = &[
    col(
        "race_id",
        Integer,
        None,
        "Carrera del instante: el race_id de races.csv.",
        EXPORT,
    ),
    col(
        "t_s",
        Number,
        Some("s"),
        "Segundos desde la salida (la picada de salida), cada segundo; el último instante es la meta y puede quedar a menos de un segundo del anterior.",
        PHYSICAL,
    ),
    col(
        "distance_m",
        Number,
        Some("m"),
        "Metros recorridos desde la salida. En un hueco del track sigue contando la línea recta.",
        PHYSICAL,
    ),
    col(
        "altitude_m",
        Number,
        Some("m"),
        "Altitud suavizada. Vacío sin altitud y en los huecos.",
        PHYSICAL,
    ),
    col(
        "speed_mps",
        Number,
        Some("m/s"),
        "Velocidad real: la distancia recorrida en ±5 s. Vacío en los huecos.",
        PHYSICAL,
    ),
    col(
        "flat_speed_mps",
        Number,
        Some("m/s"),
        "Velocidad equivalente en llano, según la pendiente de la zona. Vacío sin altitud o sin pendiente medible.",
        PHYSICAL,
    ),
    col(
        "heart_rate_bpm",
        Number,
        Some("ppm"),
        "Pulso: media de las muestras a ±5 s. Vacío sin pulso.",
        PHYSICAL,
    ),
    col(
        "cadence_spm",
        Number,
        Some("pasos/min"),
        "Cadencia: media de las muestras a ±5 s. Vacío sin cadencia.",
        PHYSICAL,
    ),
    col(
        "stride_m",
        Number,
        Some("m"),
        "Zancada: velocidad real entre cadencia. Vacío sin cadencia y parado.",
        PHYSICAL,
    ),
    col(
        "leg",
        Integer,
        None,
        "Tramo del instante: el leg de legs.csv. Vacío en los huecos.",
        PHYSICAL,
    ),
    category(
        "state",
        "Qué hace el corredor: traslado (transit), zona de ataque de la baliza (attack), parado (stopped) o sin datos (gap).",
        PHYSICAL,
        SAMPLE_STATES,
    ),
    col(
        "lat",
        Number,
        Some("grados"),
        "Latitud del instante, interpolada en el track. Solo con las coordenadas. Vacío en los huecos.",
        PHYSICAL,
    ),
    col(
        "lon",
        Number,
        Some("grados"),
        "Longitud del instante, interpolada en el track. Solo con las coordenadas. Vacío en los huecos.",
        PHYSICAL,
    ),
];

/// Cuántas columnas de [`SAMPLE_COLUMNS`] salen sin las coordenadas.
const SAMPLE_COLUMNS_WITHOUT_COORDINATES: usize = 11;

/// Las columnas de `samples.csv`, con o sin `lat` y `lon`.
pub fn sample_columns(coordinates: bool) -> &'static [Column] {
    if coordinates {
        SAMPLE_COLUMNS
    } else {
        &SAMPLE_COLUMNS[..SAMPLE_COLUMNS_WITHOUT_COORDINATES]
    }
}

/// Una tabla exportada.
#[derive(Debug, Clone, Copy)]
pub struct Table {
    pub file: &'static str,
    pub description: &'static str,
    pub columns: &'static [Column],
    /// Segundos entre filas de una serie; `None` si la tabla no es una serie.
    pub step_s: Option<f64>,
}

/// Las tablas que se exportan, en orden.
pub const TABLES: &[Table] = &[
    Table {
        file: "races.csv",
        description: "Una fila por carrera propia (sin las no presentadas).",
        columns: RACE_COLUMNS,
        step_s: None,
    },
    Table {
        file: "legs.csv",
        description: "Una fila por tramo de cada carrera de races.csv.",
        columns: LEG_COLUMNS,
        step_s: None,
    },
];

/// Las tablas de una exportación: las dos de siempre y, si se pide, `samples.csv`.
pub fn tables(options: ExportOptions) -> Vec<Table> {
    let mut tables = TABLES.to_vec();
    if options.samples {
        tables.push(Table {
            file: "samples.csv",
            description: "La serie de cada carrera de races.csv con track alineado: una fila por instante, de la salida a la meta.",
            columns: sample_columns(options.with_coordinates()),
            step_s: Some(SAMPLES_STEP_S),
        });
    }
    tables
}

/// Datos de una carrera que no salen de su informe.
#[derive(Debug, Clone, PartialEq)]
pub struct RaceMeta {
    /// Id del resultado en la base de datos.
    pub race_id: i64,
    pub event_id: i64,
    pub date: NaiveDate,
    pub kind: EventKind,
    pub format: Option<RaceFormat>,
    pub status: RaceStatus,
    pub place: Option<u16>,
    /// Si tiene el track del reloj guardado y alineado.
    pub has_track: bool,
    /// Días desde la carrera anterior ([`crate::days_off::days_since_previous`]).
    pub days_since_previous: Option<i64>,
}

/// Una fila de `races.csv`.
#[derive(Debug, Clone, PartialEq)]
pub struct RaceRecord {
    meta: RaceMeta,
    legs: usize,
    classified: Option<usize>,
    course_runners: usize,
    total_s: Option<f64>,
    lost_time_s: Option<f64>,
    error_count: usize,
    time_without_errors_s: Option<f64>,
    place_without_errors: Option<usize>,
    usual_performance: Option<f64>,
    consistency: Option<f64>,
    physical: Option<RacePhysical>,
    heart_rate_load: Option<f64>,
}

impl RaceRecord {
    /// La fila de una carrera con su informe y, si tiene track, su rendimiento físico.
    pub fn new(meta: RaceMeta, report: &RunnerReport, physical: Option<RacePhysical>) -> Self {
        let lost = &report.lost_time;
        Self {
            meta,
            legs: lost.legs.len(),
            classified: lost.place_without_errors.map(|p| p.classified),
            course_runners: report.course.valid_runners,
            total_s: lost.total_s,
            lost_time_s: lost.lost_time_s,
            error_count: lost.error_count,
            time_without_errors_s: lost.time_without_errors_s,
            place_without_errors: lost.place_without_errors.map(|p| p.place),
            usual_performance: lost.usual_performance,
            consistency: lost.consistency,
            heart_rate_load: physical
                .as_ref()
                .and_then(|p| p.heart_rate_zones.as_ref())
                .map(|z| z.load),
            physical,
        }
    }

    /// Los valores de la fila, en el orden de [`RACE_COLUMNS`].
    pub fn cells(&self) -> Vec<String> {
        let m = &self.meta;
        let mut cells = vec![
            m.race_id.to_string(),
            m.event_id.to_string(),
            m.date.to_string(),
            serde_name(&m.kind),
            m.format.map_or_else(String::new, |f| serde_name(&f)),
            status_name(m.status).to_string(),
            self.legs.to_string(),
            number(self.total_s),
            opt(m.place),
            opt(self.classified),
            self.course_runners.to_string(),
            number(self.lost_time_s),
            self.error_count.to_string(),
            number(self.time_without_errors_s),
            opt(self.place_without_errors),
            number(self.usual_performance),
            number(self.consistency),
            opt(m.days_since_previous),
            m.has_track.to_string(),
        ];
        let physical = self.physical.as_ref();
        let transit = physical.map(|p| &p.transit);
        let from_physical = |f: fn(&RacePhysical) -> Option<f64>| number(physical.and_then(f));
        let from_transit =
            |f: fn(&crate::physical::TransitStats) -> Option<f64>| number(transit.and_then(f));
        cells.extend([
            number(physical.map(|p| p.duration_s)),
            number(physical.map(|p| p.distance_m)),
            from_physical(|p| p.ascent_m),
            from_physical(|p| p.descent_m),
            number(physical.map(|p| p.moving_s)),
            number(physical.map(|p| p.stopped_s)),
            number(physical.map(|p| p.gap_s)),
            number(physical.map(|p| p.attack_s)),
            physical.map_or_else(String::new, |p| match p.speed {
                SpeedKind::Flat => "flat".into(),
                SpeedKind::Moving => "moving".into(),
            }),
            number(transit.map(|t| t.transit_s)),
            number(transit.map(|t| t.distance_m)),
            from_transit(|t| t.speed_mps),
            from_transit(|t| t.flat_speed_mps),
            from_transit(|t| t.heart_rate_bpm),
            from_transit(|t| t.efficiency_m_per_beat),
            from_transit(|t| t.cadence_spm),
            from_transit(|t| t.stride_m),
            from_physical(|p| p.heart_rate_bpm),
            from_physical(|p| p.heart_rate_max_bpm),
            from_physical(|p| p.decoupling_pct),
            from_physical(|p| p.speed_change_pct),
            from_physical(|p| p.heart_rate_change_bpm),
            from_physical(|p| p.stride_change_pct),
            from_physical(|p| p.vam_m_per_h),
            number(physical.map(|p| p.climb_s)),
            number(self.heart_rate_load),
        ]);
        cells
    }
}

/// Una fila de `legs.csv`.
#[derive(Debug, Clone, PartialEq)]
pub struct LegRecord {
    race_id: i64,
    legs: usize,
    leg: LegReport,
    tag: Option<LegTag>,
    metrics: Option<TrackMetrics>,
    days_since_previous: Option<i64>,
    previous_leg_error: Option<bool>,
}

impl LegRecord {
    /// Los valores de la fila, en el orden de [`LEG_COLUMNS`].
    pub fn cells(&self) -> Vec<String> {
        let leg = &self.leg;
        let tag = self.tag.as_ref();
        let m = self.metrics.as_ref();
        vec![
            self.race_id.to_string(),
            leg.index.to_string(),
            self.legs.to_string(),
            number(Some(leg.index as f64 / self.legs as f64)),
            (race_third(leg.index, self.legs) + 1).to_string(),
            leg.is_last.to_string(),
            leg.excluded_from_patterns.to_string(),
            number(leg.split_s),
            number(leg.reference_s),
            leg.reference_count.to_string(),
            number(leg.performance_index),
            number(leg.expected_s),
            number(leg.loss_s),
            number(leg.loss_pct),
            leg.is_error.to_string(),
            leg.calculated_error.to_string(),
            tag.and_then(|t| t.confirmation)
                .map_or_else(String::new, |c| serde_name(&c)),
            tag.and_then(|t| t.error_type.clone()).unwrap_or_default(),
            tag.and_then(|t| t.error_subtype.clone())
                .unwrap_or_default(),
            opt(tag.and_then(|t| t.effort)),
            number(m.map(|m| m.distance_m)),
            number(m.map(|m| m.straight_m)),
            number(m.and_then(|m| m.distance_ratio)),
            number(m.and_then(|m| m.moving_speed_mps)),
            number(m.and_then(|m| m.flat_speed_mps)),
            number(m.map(|m| m.moving_s)),
            number(m.map(|m| m.stopped_s)),
            number(m.and_then(|m| m.ascent_m)),
            number(m.and_then(|m| m.descent_m)),
            number(m.and_then(|m| m.heart_rate_bpm)),
            number(m.and_then(|m| m.cadence_spm)),
            number(m.and_then(|m| m.attack_s)),
            opt(self.days_since_previous),
            opt(self.previous_leg_error),
        ]
    }
}

/// Las filas de `legs.csv` de una carrera: un tramo por fila, con la etiqueta del corredor
/// (`tags`: `(tramo, etiqueta)`, con el tramo desde 1) y, con track, las métricas de su tramo.
pub fn leg_records(
    race_id: i64,
    report: &RunnerReport,
    tags: &[(usize, LegTag)],
    metrics: Option<&[LegMetrics]>,
    days_since_previous: Option<i64>,
) -> Vec<LegRecord> {
    let legs = &report.lost_time.legs;
    legs.iter()
        .enumerate()
        .map(|(i, leg)| {
            // El tramo anterior del recorrido; sin pérdida (sin split) no se sabe si fue error.
            let previous_leg_error = i
                .checked_sub(1)
                .and_then(|p| legs.get(p))
                .filter(|prev| prev.loss_s.is_some())
                .map(|prev| prev.is_error);
            LegRecord {
                race_id,
                legs: legs.len(),
                leg: leg.clone(),
                tag: tags
                    .iter()
                    .find(|(l, _)| *l == leg.index)
                    .map(|(_, t)| t.clone()),
                metrics: metrics
                    .and_then(|ms| {
                        ms.iter()
                            .find(|m| m.index == leg.index && m.from == leg.from && m.to == leg.to)
                    })
                    .and_then(|m| m.track.clone()),
                days_since_previous,
                previous_leg_error,
            }
        })
        .collect()
}

/// Las filas de `samples.csv` de una carrera: un instante de `series` por fila, en las columnas
/// de [`sample_columns`]. Con `positions` (una por instante, de
/// [`crate::physical_series::series_positions`]) añade `lat` y `lon`. Los números se redondean
/// para que el fichero no pese de más: el tiempo a milisegundos, las distancias y la altitud a
/// centímetros, las velocidades y la zancada a milímetros, el pulso y la cadencia a décimas y las
/// coordenadas a 6 decimales (unos 0,1 m).
pub fn sample_rows(
    race_id: i64,
    series: &RaceSeries,
    positions: Option<&[Option<(f64, f64)>]>,
) -> Vec<Vec<String>> {
    series
        .samples
        .iter()
        .enumerate()
        .map(|(i, sample)| {
            let mut cells = vec![
                race_id.to_string(),
                rounded(Some(sample.t_s), 3),
                rounded(Some(sample.distance_m), 2),
                rounded(sample.altitude_m, 2),
                rounded(sample.speed_mps, 3),
                rounded(sample.flat_speed_mps, 3),
                rounded(sample.heart_rate_bpm, 1),
                rounded(sample.cadence_spm, 1),
                rounded(sample.stride_m, 3),
                opt(sample.leg),
                match sample.state {
                    SeriesState::Transit => "transit",
                    SeriesState::Attack => "attack",
                    SeriesState::Stopped => "stopped",
                    SeriesState::Gap => "gap",
                }
                .to_string(),
            ];
            if let Some(positions) = positions {
                let position = positions.get(i).copied().flatten();
                cells.push(rounded(position.map(|(lat, _)| lat), 6));
                cells.push(rounded(position.map(|(_, lon)| lon), 6));
            }
            cells
        })
        .collect()
}

/// Un número redondeado a `decimals` decimales, como [`number`]; sin «-0».
fn rounded(value: Option<f64>, decimals: i32) -> String {
    let scale = 10f64.powi(decimals);
    number(value.map(|v| (v * scale).round() / scale + 0.0))
}

/// Un número como texto con punto decimal; vacío si falta o no es finito.
fn number(value: Option<f64>) -> String {
    value
        .filter(|v| v.is_finite())
        .map_or_else(String::new, |v| v.to_string())
}

/// Un valor que se escribe con su `Display`, o vacío.
fn opt<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(String::new, |v| v.to_string())
}

/// El nombre en `snake_case` de un valor que se serializa como una cadena.
fn serde_name<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(name)) => name,
        _ => String::new(),
    }
}

fn status_name(status: RaceStatus) -> &'static str {
    match status {
        RaceStatus::Ok => "ok",
        RaceStatus::NotClassified => "not_classified",
        RaceStatus::DidNotStart => "did_not_start",
        RaceStatus::Unknown(_) => "unknown",
    }
}

/// Escribe un valor como campo CSV (RFC 4180): entre comillas si lleva coma, comillas o salto de
/// línea, y con las comillas dobladas.
fn write_field(out: &mut impl Write, field: &str) -> io::Result<()> {
    if field.contains([',', '"', '\n', '\r']) {
        write!(out, "\"{}\"", field.replace('"', "\"\""))
    } else {
        out.write_all(field.as_bytes())
    }
}

fn write_record<S: AsRef<str>>(out: &mut impl Write, cells: &[S]) -> io::Result<()> {
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            out.write_all(b",")?;
        }
        write_field(out, cell.as_ref())?;
    }
    out.write_all(b"\n")
}

/// Escribe una tabla CSV: la cabecera con los nombres de `columns` y una línea por fila. UTF-8,
/// sin BOM, líneas acabadas en `\n`. Una fila con otro número de valores que columnas es un
/// error.
pub fn write_csv(
    out: &mut impl Write,
    columns: &[Column],
    rows: &[Vec<String>],
) -> Result<(), ExportError> {
    write_csv_header(out, columns)?;
    write_csv_rows(out, columns, rows)
}

/// Escribe la cabecera de una tabla CSV. Con [`write_csv_rows`] permite escribir una tabla grande
/// por partes, sin tener todas las filas a la vez.
pub fn write_csv_header(out: &mut impl Write, columns: &[Column]) -> Result<(), ExportError> {
    let header: Vec<&str> = columns.iter().map(|c| c.name).collect();
    write_record(out, &header)?;
    Ok(())
}

/// Escribe filas de una tabla CSV, con el formato de [`write_csv`].
pub fn write_csv_rows(
    out: &mut impl Write,
    columns: &[Column],
    rows: &[Vec<String>],
) -> Result<(), ExportError> {
    for row in rows {
        if row.len() != columns.len() {
            return Err(ExportError::RowLength {
                expected: columns.len(),
                found: row.len(),
            });
        }
        write_record(out, row)?;
    }
    Ok(())
}

/// El `schema.json`: la versión del formato, las convenciones y cada columna de cada tabla
/// exportada.
pub fn schema_json(options: ExportOptions) -> Value {
    let tables: Vec<Value> = tables(options)
        .iter()
        .map(|t| {
            let mut table = json!({
                "file": t.file,
                "description": t.description,
                "columns": t.columns,
            });
            if let (Some(step_s), Some(object)) = (t.step_s, table.as_object_mut()) {
                object.insert("step_s".into(), json!(step_s));
            }
            table
        })
        .collect();
    json!({
        "format_version": FORMAT_VERSION,
        "conventions": {
            "encoding": "UTF-8, sin BOM",
            "separator": ",",
            "decimal_separator": ".",
            "line_ending": "\\n",
            "dates": "ISO 8601 (AAAA-MM-DD)",
            "booleans": "true / false",
            "missing": "campo vacío",
        },
        "tables": tables,
    })
}

/// Lo que se ha exportado, para enseñarlo y para el `LEEME.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ExportSummary {
    pub races: usize,
    pub legs: usize,
    /// Carreras con filas en `samples.csv` (0 si no se exporta).
    pub sample_races: usize,
    /// Filas de `samples.csv` (0 si no se exporta).
    pub sample_rows: usize,
}

/// El `LEEME.md` de la carpeta exportada.
pub fn readme_md(
    summary: ExportSummary,
    options: ExportOptions,
    generated_at: DateTime<Utc>,
) -> String {
    let mut text = format!(
        "# Datos exportados de Tramos\n\n\
         Exportados el {} con la versión {} del formato: {} carreras y {} tramos. Los ha generado \
         la app Tramos (<https://github.com/Mendana/tramos>) con sus propios cálculos; el formato \
         completo está en `docs/exportar-datos.md` del repositorio.\n\n\
         ## Ficheros\n\n\
         - `races.csv`: una fila por carrera tuya.\n\
         - `legs.csv`: una fila por tramo. Se une con `races.csv` por `race_id`.\n",
        generated_at.format("%Y-%m-%d %H:%M UTC"),
        FORMAT_VERSION,
        summary.races,
        summary.legs,
    );
    if options.samples {
        text.push_str(&format!(
            "- `samples.csv`: la serie de cada carrera con track, una fila por segundo ({} filas de \
             {} carreras). Se une con `races.csv` por `race_id` y con `legs.csv` por `race_id` y \
             `leg`.\n",
            summary.sample_rows, summary.sample_races,
        ));
    }
    text.push_str(
        "- `schema.json`: cada columna con su tipo, unidad, descripción y el documento de `docs/` \
         donde se define, en formato legible por programas.\n\n\
         ## Formato\n\n\
         CSV en UTF-8 sin BOM, separado por comas, con punto decimal, fechas ISO 8601 \
         (AAAA-MM-DD), `true` o `false` en las columnas de sí o no y el campo vacío para lo que \
         no hay (sin FIT, sin pulso, sin etiqueta…). Los vacíos no son ceros.\n\n\
         ## Privacidad\n\n",
    );
    text.push_str(if options.with_coordinates() {
        "No hay nombres de nadie, ni notas, ni resultados de otros corredores (solo la referencia \
         de cada tramo, que es un agregado). Pero `samples.csv` lleva `lat` y `lon` de cada \
         instante: dicen por dónde has corrido y, con la fecha, dónde estabas. Aun así son tus \
         datos: compártelos con cuidado, y más este fichero.\n\n"
    } else {
        "No hay nombres de nadie, ni notas, ni coordenadas, ni resultados de otros corredores \
         (solo la referencia de cada tramo, que es un agregado). Aun así son tus datos: fechas, \
         pulso y cómo corres. Compártelos con cuidado.\n\n"
    });
    text.push_str(
        "## Cargar los datos con pandas\n\n\
         ```python\n\
         import pandas as pd\n\n\
         races = pd.read_csv(\"races.csv\", parse_dates=[\"date\"])\n\
         legs = pd.read_csv(\"legs.csv\")\n\
         data = legs.merge(races, on=\"race_id\", suffixes=(\"\", \"_race\"))\n",
    );
    if options.samples {
        text.push_str("samples = pd.read_csv(\"samples.csv\")\n");
    }
    text.push_str("```\n");
    for table in tables(options) {
        text.push_str(&format!(
            "\n## Columnas de `{}`\n\n| Columna | Tipo | Unidad | Qué es | Definición |\n| --- | --- | --- | --- | --- |\n",
            table.file
        ));
        for c in table.columns {
            let kind = serde_name(&c.kind);
            text.push_str(&format!(
                "| `{}` | {} | {} | {} | `{}` |\n",
                c.name,
                kind,
                c.unit.unwrap_or(""),
                c.description.replace('|', "/"),
                c.source,
            ));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ControlCode;
    use crate::physical::{HeartRateZones, TransitStats};
    use crate::physical_series::SeriesSample;
    use crate::runner_report::{CourseSummary, PlaceWithoutErrors, RunnerLostTime};
    use crate::taxonomy::Confirmation;

    fn leg(index: usize, split: Option<f64>, error: bool) -> LegReport {
        LegReport {
            index,
            from: 30 + index as ControlCode,
            to: 31 + index as ControlCode,
            split_s: split,
            elapsed_s: None,
            place: Some(3),
            reference_s: split.map(|s| s - 10.0),
            reference_count: 5,
            valid_splits: 20,
            performance_index: split.map(|s| (s - 10.0) / s),
            expected_s: split.map(|s| s - 20.0),
            loss_s: split.map(|_| 20.0),
            loss_pct: split.map(|_| 50.0),
            is_error: error,
            calculated_error: !error,
            gain_s: None,
            cumulative_gain_s: None,
            ideal_elapsed_s: None,
            behind_ideal_s: None,
            is_last: index == 3,
            short_reference: false,
            excluded_from_patterns: index == 3,
        }
    }

    fn report() -> RunnerReport {
        RunnerReport {
            course: CourseSummary {
                controls: vec![],
                classes: vec![],
                valid_runners: 12,
                weak_reference: false,
            },
            lost_time: RunnerLostTime {
                total_s: Some(600.5),
                usual_performance: Some(0.9),
                lost_time_s: Some(20.0),
                error_count: 1,
                time_without_errors_s: Some(580.5),
                place_without_errors: Some(PlaceWithoutErrors {
                    place: 2,
                    classified: 18,
                }),
                ideal_time_s: None,
                behind_ideal_s: None,
                losing_streaks: vec![],
                consistency: Some(0.125),
                legs: vec![
                    leg(1, Some(100.0), false),
                    leg(2, Some(200.0), true),
                    leg(3, None, false),
                ],
            },
        }
    }

    fn meta() -> RaceMeta {
        RaceMeta {
            race_id: 7,
            event_id: 4,
            date: "2026-10-03".parse().unwrap(),
            kind: EventKind::Race,
            format: Some(RaceFormat::Sprint),
            status: RaceStatus::Ok,
            place: Some(16),
            has_track: false,
            days_since_previous: Some(12),
        }
    }

    fn names(columns: &[Column]) -> Vec<&'static str> {
        columns.iter().map(|c| c.name).collect()
    }

    #[test]
    fn column_names_are_unique_snake_case() {
        for table in TABLES {
            let mut seen = names(table.columns);
            assert!(seen.iter().all(|n| {
                !n.is_empty()
                    && n.chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            }));
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(seen.len(), table.columns.len(), "{}", table.file);
            assert!(table.columns.iter().all(|c| c.source.starts_with("docs/")));
        }
    }

    /// Una carrera sin track: lo calculado a mano y vacío en lo físico.
    #[test]
    fn race_row_without_track_has_empty_physical_columns() {
        let record = RaceRecord::new(meta(), &report(), None);
        let cells = record.cells();
        assert_eq!(cells.len(), RACE_COLUMNS.len());
        let by_name = |name: &str| {
            let i = RACE_COLUMNS.iter().position(|c| c.name == name).unwrap();
            cells[i].as_str()
        };
        assert_eq!(by_name("race_id"), "7");
        assert_eq!(by_name("date"), "2026-10-03");
        assert_eq!(by_name("kind"), "race");
        assert_eq!(by_name("format"), "sprint");
        assert_eq!(by_name("status"), "ok");
        assert_eq!(by_name("legs"), "3");
        assert_eq!(by_name("total_s"), "600.5");
        assert_eq!(by_name("place"), "16");
        assert_eq!(by_name("classified"), "18");
        assert_eq!(by_name("course_runners"), "12");
        assert_eq!(by_name("lost_time_s"), "20");
        assert_eq!(by_name("place_without_errors"), "2");
        assert_eq!(by_name("consistency"), "0.125");
        assert_eq!(by_name("days_since_previous_race"), "12");
        assert_eq!(by_name("has_track"), "false");
        let from = RACE_COLUMNS
            .iter()
            .position(|c| c.name == "track_duration_s")
            .unwrap();
        assert!(cells[from..].iter().all(String::is_empty));
    }

    fn physical() -> RacePhysical {
        let transit = TransitStats {
            transit_s: 500.0,
            distance_m: 2000.0,
            speed_mps: Some(4.0),
            flat_speed_mps: Some(4.5),
            heart_rate_bpm: Some(150.0),
            efficiency_m_per_beat: Some(1.8),
            cadence_spm: None,
            stride_m: None,
        };
        RacePhysical {
            duration_s: 600.0,
            distance_m: 2400.0,
            ascent_m: Some(30.0),
            descent_m: None,
            moving_s: 520.0,
            stopped_s: 40.0,
            gap_s: 0.0,
            attack_s: 40.0,
            speed: SpeedKind::Flat,
            transit,
            heart_rate_bpm: Some(148.5),
            heart_rate_max_bpm: Some(175.0),
            halves: None,
            decoupling_pct: Some(9.08),
            speed_change_pct: None,
            heart_rate_change_bpm: None,
            stride_change_pct: None,
            blocks: vec![],
            by_grade: vec![],
            vam_m_per_h: Some(1440.0),
            climb_s: 100.0,
            heart_rate_zones: Some(HeartRateZones {
                seconds: vec![60.0, 120.0],
                load: 8.03,
            }),
        }
    }

    /// Con track, las columnas físicas salen de `RacePhysical` en su sitio.
    #[test]
    fn race_row_with_track_takes_the_physical_numbers() {
        let mut m = meta();
        m.has_track = true;
        let cells = RaceRecord::new(m, &report(), Some(physical())).cells();
        assert_eq!(cells.len(), RACE_COLUMNS.len());
        let tail = [
            "true", "600", "2400", "30", "", "520", "40", "0", "40", "flat", "500", "2000", "4",
            "4.5", "150", "1.8", "", "", "148.5", "175", "9.08", "", "", "", "1440", "100", "8.03",
        ];
        let from = RACE_COLUMNS
            .iter()
            .position(|c| c.name == "has_track")
            .unwrap();
        assert_eq!(&cells[from..], &tail);
    }

    fn tag() -> LegTag {
        LegTag {
            confirmation: Some(Confirmation::Error),
            error_type: Some("direction".into()),
            error_subtype: Some("bearing".into()),
            effort: Some(8),
            note: Some("Me llamo Ana, y me perdí".into()),
            ..LegTag::default()
        }
    }

    fn track_metrics() -> TrackMetrics {
        TrackMetrics {
            duration_s: 200.0,
            distance_m: 700.0,
            straight_m: 500.0,
            distance_ratio: Some(1.4),
            moving_s: 180.0,
            stopped_s: 20.0,
            gap_s: 0.0,
            moving_speed_mps: Some(3.5),
            flat_speed_mps: None,
            ascent_m: Some(12.5),
            descent_m: Some(3.0),
            heart_rate_bpm: Some(160.0),
            cadence_spm: None,
            attack_s: Some(15.0),
        }
    }

    #[test]
    fn leg_rows_have_tag_metrics_and_context() {
        let lost = report();
        let metrics = vec![LegMetrics {
            index: 2,
            from: 32,
            to: 33,
            track: Some(track_metrics()),
            missing: None,
        }];
        let rows = leg_records(7, &lost, &[(2, tag())], Some(&metrics), Some(12));
        assert_eq!(rows.len(), 3);
        let cells: Vec<Vec<String>> = rows.iter().map(LegRecord::cells).collect();
        assert!(cells.iter().all(|c| c.len() == LEG_COLUMNS.len()));

        // Tramo 1: sin etiqueta ni track, sin tramo anterior.
        assert_eq!(
            cells[0].join(","),
            "7,1,3,0.3333333333333333,1,false,false,100,90,5,0.9,80,20,50,false,true,,,,,,,,,,,,,,,,,12,"
        );
        // Tramo 2: error revisado con etiqueta, con métricas; el anterior no fue error.
        assert_eq!(
            cells[1].join(","),
            "7,2,3,0.6666666666666666,2,false,false,200,190,5,0.95,180,20,50,true,false,error,direction,bearing,8,700,500,1.4,3.5,,180,20,12.5,3,160,,15,12,false"
        );
        // Tramo 3: el último, sin split ni pérdida; el anterior fue error.
        assert_eq!(cells[2][5], "true");
        assert_eq!(cells[2][6], "true");
        assert_eq!(cells[2][7], "");
        assert_eq!(cells[2][13], "");
        assert_eq!(cells[2].last().map(String::as_str), Some("true"));
        assert_eq!(cells[2][4], "3");
    }

    /// La nota libre (puede llevar nombres) no sale nunca.
    #[test]
    fn the_free_note_never_leaves() {
        let rows = leg_records(7, &report(), &[(2, tag())], None, None);
        for row in rows {
            assert!(row.cells().iter().all(|c| !c.contains("Ana")));
        }
    }

    #[test]
    fn a_leg_after_one_without_split_has_no_previous_error() {
        let mut lost = report();
        lost.lost_time.legs[0].split_s = None;
        lost.lost_time.legs[0].loss_s = None;
        let rows = leg_records(1, &lost, &[], None, None);
        assert_eq!(rows[1].cells().last().map(String::as_str), Some(""));
    }

    #[test]
    fn third_of_the_race_is_one_to_three() {
        // 21 tramos: 1–7, 8–14 y 15–21.
        let thirds: Vec<usize> = [1, 7, 8, 14, 15, 21]
            .iter()
            .map(|&i| race_third(i, 21) + 1)
            .collect();
        assert_eq!(thirds, [1, 1, 2, 2, 3, 3]);
    }

    #[test]
    fn csv_escapes_commas_quotes_and_newlines() {
        let mut out = Vec::new();
        let columns = &LEG_COLUMNS[..2];
        let rows = vec![
            vec!["a,b".to_string(), "dice \"hola\"".to_string()],
            vec!["línea\nnueva".to_string(), String::new()],
            vec!["ñ".to_string(), "3.5".to_string()],
        ];
        write_csv(&mut out, columns, &rows).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "race_id,leg\n\"a,b\",\"dice \"\"hola\"\"\"\n\"línea\nnueva\",\nñ,3.5\n"
        );
    }

    #[test]
    fn csv_rejects_a_row_with_the_wrong_length() {
        let mut out = Vec::new();
        let err = write_csv(&mut out, &LEG_COLUMNS[..2], &[vec!["1".into()]]).unwrap_err();
        assert!(matches!(
            err,
            ExportError::RowLength {
                expected: 2,
                found: 1
            }
        ));
    }

    #[test]
    fn numbers_use_a_decimal_point_and_skip_non_finite() {
        assert_eq!(number(Some(1234.5)), "1234.5");
        assert_eq!(number(Some(-0.25)), "-0.25");
        assert_eq!(number(Some(3.0)), "3");
        assert_eq!(number(Some(f64::NAN)), "");
        assert_eq!(number(Some(f64::INFINITY)), "");
        assert_eq!(number(None), "");
    }

    #[test]
    fn schema_lists_every_column_with_its_source() {
        let schema = schema_json(ExportOptions::default());
        assert_eq!(schema["format_version"], FORMAT_VERSION);
        let tables = schema["tables"].as_array().unwrap();
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0]["file"], "races.csv");
        let columns = tables[1]["columns"].as_array().unwrap();
        assert_eq!(columns.len(), LEG_COLUMNS.len());
        let split = columns.iter().find(|c| c["name"] == "split_s").unwrap();
        assert_eq!(split["type"], "number");
        assert_eq!(split["unit"], "s");
        assert_eq!(split["source"], "docs/tiempo-perdido.md");
        assert!(split.get("allowed_values").is_none());
        let races = tables[0]["columns"].as_array().unwrap();
        let kind = races.iter().find(|c| c["name"] == "kind").unwrap();
        assert_eq!(kind["allowed_values"], json!(["race", "training"]));
        assert!(kind.get("unit").is_none());
    }

    const WITH_SAMPLES: ExportOptions = ExportOptions {
        samples: true,
        coordinates: false,
    };
    const WITH_COORDINATES: ExportOptions = ExportOptions {
        samples: true,
        coordinates: true,
    };

    /// `samples.csv` solo está en el esquema si se pide, y con él su paso; `lat` y `lon` solo con
    /// las coordenadas.
    #[test]
    fn schema_has_samples_only_when_asked() {
        let files = |options| -> Vec<String> {
            schema_json(options)["tables"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t["file"].as_str().unwrap().to_string())
                .collect()
        };
        assert_eq!(files(ExportOptions::default()), ["races.csv", "legs.csv"]);
        // Las coordenadas sin la serie no hacen nada.
        let only_coordinates = ExportOptions {
            samples: false,
            coordinates: true,
        };
        assert_eq!(files(only_coordinates), ["races.csv", "legs.csv"]);
        assert_eq!(
            files(WITH_SAMPLES),
            ["races.csv", "legs.csv", "samples.csv"]
        );

        let column_names = |options| -> Vec<String> {
            let schema = schema_json(options);
            schema["tables"][2]["columns"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["name"].as_str().unwrap().to_string())
                .collect()
        };
        assert_eq!(
            column_names(WITH_SAMPLES),
            [
                "race_id",
                "t_s",
                "distance_m",
                "altitude_m",
                "speed_mps",
                "flat_speed_mps",
                "heart_rate_bpm",
                "cadence_spm",
                "stride_m",
                "leg",
                "state"
            ]
        );
        assert_eq!(
            &column_names(WITH_COORDINATES)[11..],
            ["lat".to_string(), "lon".to_string()]
        );
        let schema = schema_json(WITH_SAMPLES);
        assert_eq!(schema["tables"][2]["step_s"], 1.0);
        assert_eq!(
            schema["tables"][2]["columns"][10]["allowed_values"],
            json!(["transit", "attack", "stopped", "gap"])
        );
        assert!(schema["tables"][0].get("step_s").is_none());
    }

    fn sample(t_s: f64, state: SeriesState) -> SeriesSample {
        SeriesSample {
            t_s,
            distance_m: 4.0 * t_s,
            altitude_m: Some(500.0),
            speed_mps: Some(4.0),
            flat_speed_mps: Some(4.5),
            heart_rate_bpm: Some(150.0),
            cadence_spm: Some(180.0),
            stride_m: Some(4.0 * 60.0 / 180.0),
            leg: Some(1),
            state,
        }
    }

    fn series() -> RaceSeries {
        let gap = SeriesSample {
            distance_m: 8.0,
            altitude_m: None,
            speed_mps: None,
            flat_speed_mps: None,
            heart_rate_bpm: None,
            cadence_spm: None,
            stride_m: None,
            leg: None,
            ..sample(2.0, SeriesState::Gap)
        };
        RaceSeries {
            step_s: SAMPLES_STEP_S,
            duration_s: 2.5,
            samples: vec![
                sample(0.0, SeriesState::Transit),
                sample(1.0, SeriesState::Attack),
                gap,
                SeriesSample {
                    heart_rate_bpm: None,
                    speed_mps: Some(1.0 / 3.0),
                    ..sample(2.5, SeriesState::Stopped)
                },
            ],
            legs: vec![],
            controls: vec![],
        }
    }

    /// Una fila por instante, con los números de la serie en su sitio y los huecos vacíos.
    #[test]
    fn sample_rows_take_the_series_numbers_in_order() {
        let rows = sample_rows(7, &series(), None);
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|r| r.len() == sample_columns(false).len()));
        assert_eq!(rows[0].join(","), "7,0,0,500,4,4.5,150,180,1.333,1,transit");
        assert_eq!(rows[1].join(","), "7,1,4,500,4,4.5,150,180,1.333,1,attack");
        // Un hueco: distancia y estado, y nada más.
        assert_eq!(rows[2].join(","), "7,2,8,,,,,,,,gap");
        // Redondeado: 1/3 m/s a milímetros y t = 2,5 s.
        assert_eq!(
            rows[3].join(","),
            "7,2.5,10,500,0.333,4.5,,180,1.333,1,stopped"
        );
    }

    #[test]
    fn sample_rows_add_the_coordinates_only_when_given() {
        let positions = vec![
            Some((40.123_456_789, -4.5)),
            Some((40.0, -4.000_000_4)),
            None,
            Some((-0.000_000_1, 0.0)),
        ];
        let rows = sample_rows(7, &series(), Some(&positions));
        assert!(rows.iter().all(|r| r.len() == sample_columns(true).len()));
        let tails: Vec<String> = rows.iter().map(|r| r[11..].join(",")).collect();
        // Seis decimales; sin «-0»; vacío en el hueco.
        assert_eq!(tails, ["40.123457,-4.5", "40,-4", ",", "0,0"]);
        // Sin posiciones no hay ni las columnas.
        assert!(
            sample_rows(7, &series(), None)
                .iter()
                .all(|r| r.len() == 11)
        );
    }

    #[test]
    fn csv_can_be_written_in_parts() {
        let columns = sample_columns(false);
        let mut parts = Vec::new();
        write_csv_header(&mut parts, columns).unwrap();
        write_csv_rows(&mut parts, columns, &sample_rows(7, &series(), None)[..2]).unwrap();
        write_csv_rows(&mut parts, columns, &sample_rows(8, &series(), None)[2..]).unwrap();
        let mut whole = Vec::new();
        let mut rows = sample_rows(7, &series(), None)[..2].to_vec();
        rows.extend_from_slice(&sample_rows(8, &series(), None)[2..]);
        write_csv(&mut whole, columns, &rows).unwrap();
        assert_eq!(parts, whole);
        let text = String::from_utf8(parts).unwrap();
        assert!(text.starts_with("race_id,t_s,distance_m,altitude_m,"));
        assert_eq!(text.lines().count(), 1 + 4);
    }

    #[test]
    fn readme_lists_the_summary_and_the_columns() {
        let at = "2026-10-25T10:30:00Z".parse().unwrap();
        let summary = ExportSummary {
            races: 3,
            legs: 61,
            sample_races: 2,
            sample_rows: 5400,
        };
        let text = readme_md(summary, ExportOptions::default(), at);
        assert!(text.contains("2026-10-25 10:30 UTC"));
        assert!(text.contains("3 carreras y 61 tramos"));
        for table in TABLES {
            for c in table.columns {
                assert!(text.contains(&format!("| `{}` |", c.name)), "{}", c.name);
            }
        }
        // Sin la serie, el LEEME no la menciona.
        assert!(!text.contains("samples"));
        assert!(text.contains("ni coordenadas"));
    }

    /// Con la serie, el LEEME la cuenta y avisa de las coordenadas solo si salen.
    #[test]
    fn readme_describes_the_series_and_warns_about_coordinates() {
        let at = "2026-10-25T10:30:00Z".parse().unwrap();
        let summary = ExportSummary {
            races: 3,
            legs: 61,
            sample_races: 2,
            sample_rows: 5400,
        };
        let plain = readme_md(summary, WITH_SAMPLES, at);
        assert!(plain.contains("`samples.csv`: la serie de cada carrera"));
        assert!(plain.contains("5400 filas de 2 carreras"));
        assert!(plain.contains("samples = pd.read_csv"));
        assert!(plain.contains("ni coordenadas"));
        assert!(!plain.contains("| `lat` |"));
        for c in sample_columns(false) {
            assert!(plain.contains(&format!("| `{}` |", c.name)), "{}", c.name);
        }

        let located = readme_md(summary, WITH_COORDINATES, at);
        assert!(located.contains("| `lat` |") && located.contains("| `lon` |"));
        assert!(located.contains("dicen por dónde has corrido"));
        assert!(!located.contains("ni coordenadas"));
    }

    /// El documento del formato nombra todas las columnas y la versión.
    #[test]
    fn the_doc_names_every_column() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/exportar-datos.md");
        let doc = std::fs::read_to_string(path).unwrap();
        assert!(doc.contains(&format!("`format_version`: {FORMAT_VERSION}")));
        // Cada tabla tiene su sección «### `fichero.csv`», en el orden de `tables`; con las
        // coordenadas salen todas las columnas de `samples.csv`, también `lat` y `lon`.
        let (_, rest) = doc.split_once("### `races.csv`").unwrap();
        let (races, rest) = rest.split_once("### `legs.csv`").unwrap();
        let (legs, samples) = rest.split_once("### `samples.csv`").unwrap();
        let all = tables(WITH_COORDINATES);
        assert_eq!(all.len(), 3);
        for (table, section) in all.iter().zip([races, legs, samples]) {
            for c in table.columns {
                assert!(
                    section.contains(&format!("| `{}` |", c.name)),
                    "{} falta en la sección de {} de docs/exportar-datos.md",
                    c.name,
                    table.file
                );
            }
        }
    }
}
