//! Alineación del track del reloj con las picadas del cronometraje.
//!
//! El reloj (FIT, UTC) y el cronometraje (.spl, hora local ya convertida a UTC por su lector)
//! pueden diferir unos segundos. [`align`] comprueba que el track cubre la carrera, estima ese
//! desfase fino y sitúa cada picada en el track. El método completo está en
//! `docs/alineacion.md`. Resumen:
//!
//! - **Convenio**: `instante en el track = instante de la picada + offset_s`. Un reloj que va
//!   7 s adelantado respecto al cronometraje da `offset_s = +7`.
//! - **Señal**: el track se remuestrea a 1 Hz y en cada segundo se mide cuánto "parece una
//!   baliza": cambio de dirección, bajada de velocidad y bajada de cadencia respecto a lo
//!   habitual del corredor en esa carrera. Se suaviza con un núcleo triangular.
//! - **Coste**: para cada desfase candidato, la media de la señal (menos su valor medio en la
//!   carrera) en los instantes de las picadas desplazados. Se maximiza en una rejilla de 1 s en
//!   ±`max_offset_s` y se refina con una parábola.
//! - La salida no se usa para estimar el desfase (en el .spl suele ser la hora asignada, no una
//!   marca física) y las picadas sin hora se ignoran.

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::{ControlCode, FINISH_CODE, RaceResult, START_CODE, Track};

/// Radio medio de la Tierra en metros (para proyectar lat/lon a un plano local).
const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Peso del cambio de dirección en la señal de "baliza".
const TURN_WEIGHT: f64 = 0.5;
/// Peso de la bajada de velocidad.
const SPEED_WEIGHT: f64 = 0.35;
/// Peso de la bajada de cadencia.
const CADENCE_WEIGHT: f64 = 0.15;
/// Por debajo de esta velocidad mediana (m/s) en la carrera no hay señal de velocidad útil.
const MIN_REFERENCE_SPEED_MPS: f64 = 0.5;
/// Fracción mínima de segundos de carrera con cadencia para usarla como señal.
const MIN_CADENCE_COVERAGE: f64 = 0.5;
/// Semiancho (s) de la búsqueda del desfase propio de cada picada alrededor del global.
const LOCAL_WINDOW_S: i64 = 15;
/// Una picada "apoya" el desfase global si su desfase propio está a esta distancia (s) o menos.
const SUPPORT_TOLERANCE_S: i64 = 3;
/// Los desfases a menos de esta distancia (s) del mejor son el mismo pico, no una alternativa.
const SECOND_PEAK_EXCLUSION_S: i64 = 10;
/// Un segundo pico con al menos esta fracción de la altura del mejor hace el desfase ambiguo.
const AMBIGUOUS_PEAK_RATIO: f64 = 0.85;
/// Apoyo típico por azar (con picadas de otro corredor sale 0,2–0,6): por debajo, confianza 0.
const SUPPORT_FLOOR: f64 = 0.4;
/// Apoyo a partir del cual el término del apoyo es 1.
const SUPPORT_FULL: f64 = 0.8;
/// Separación a partir de la cual el término de la separación es 1.
const MARGIN_FULL: f64 = 0.6;
/// Distancia al límite de la búsqueda (s) a partir de la cual se avisa.
const SEARCH_LIMIT_MARGIN_S: f64 = 1.0;
/// Mayor `max_offset_s` aceptado: más allá no es un desfase fino, es otro problema.
const MAX_SEARCH_S: u32 = 600;
/// Una picada que cae fuera del track por menos de esto (s) se sitúa en su primer o su último
/// punto: el corredor que para el reloj al picar la meta deja el track acabando en ella.
pub const EDGE_SNAP_S: f64 = 2.0;
/// Mayor ventana de giro o de suavizado aceptada (s).
const MAX_WINDOW_S: u32 = 60;

/// Parámetros de la alineación. Los valores por defecto son los de `docs/alineacion.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AlignmentOptions {
    /// Semiancho de la búsqueda del desfase, en segundos (rejilla de 1 s).
    pub max_offset_s: u32,
    /// Ventana (s) antes y después de cada instante para medir el cambio de dirección.
    pub turn_window_s: u32,
    /// Semiancho (s) del núcleo triangular con el que se suaviza la señal.
    pub smoothing_s: u32,
    /// Desplazamiento (m) en la ventana de giro por debajo del cual el rumbo no es fiable
    /// (ruido del GPS parado) y el giro pesa menos.
    pub min_displacement_m: f64,
    /// Dos puntos seguidos separados más que esto (s) forman un hueco: no hay señal dentro.
    pub max_gap_s: f64,
    /// Margen (s) que puede faltar al principio o al final de la carrera sin avisar.
    pub coverage_tolerance_s: f64,
    /// Un desfase mayor que esto (en valor absoluto, s) produce un aviso.
    pub large_offset_s: f64,
    /// Mínimo de picadas útiles para estimar el desfase; con menos se usa 0.
    pub min_controls: usize,
    /// Con menos picadas útiles que esto (pero al menos `min_controls`) se avisa.
    pub few_controls: usize,
    /// Por debajo de esta confianza (0–1) se avisa.
    pub low_confidence: f64,
    /// Usar también la salida para estimar el desfase. Por defecto no: en el .spl la hora de
    /// salida suele ser la asignada, no una marca física.
    pub use_start_punch: bool,
}

impl Default for AlignmentOptions {
    fn default() -> Self {
        Self {
            max_offset_s: 60,
            turn_window_s: 5,
            smoothing_s: 3,
            min_displacement_m: 5.0,
            max_gap_s: 10.0,
            coverage_tolerance_s: 5.0,
            large_offset_s: 30.0,
            min_controls: 3,
            few_controls: 5,
            low_confidence: 0.5,
            use_start_punch: false,
        }
    }
}

/// Desplazamientos que se prueban cuando el track no se solapa con la carrera, en orden: una y
/// dos horas en cada sentido (horario de verano o zona horaria mal elegida).
pub const SUGGESTED_SHIFTS_S: [i64; 4] = [3600, -3600, 7200, -7200];

/// Final del mensaje de `TrackOutsideRace`, con la pista si la hay.
fn shift_hint(shift_s: Option<i64>) -> String {
    match shift_s {
        Some(shift) => {
            let (sign, side) = if shift > 0 {
                ("+", "por detrás")
            } else {
                ("-", "por delante")
            };
            format!(
                "con {sign}{} h sí se solaparía (el cronometraje va {side} del reloj): \
                 ¿la hora está mal convertida (horario de verano o zona horaria)?",
                shift.abs() / 3600
            )
        }
        None => "¿es el FIT de otra carrera o la hora está mal convertida?".into(),
    }
}

/// Errores que impiden alinear.
#[derive(Debug, Error, PartialEq)]
pub enum AlignmentError {
    #[error("opciones de alineación inválidas: {0}")]
    InvalidOptions(&'static str),

    #[error("el track no tiene puntos")]
    EmptyTrack,

    #[error("el resultado no tiene ninguna picada con hora")]
    NoPunchTimes,

    #[error("no hay ventana de carrera: hacen falta dos picadas con hora (salida y meta) en orden")]
    NoRaceWindow,

    #[error(
        "el track ({track_start} – {track_end}) no se solapa con la carrera ({race_start} – \
         {race_finish}) ni con ±{max_offset_s} s de desfase: {hint}",
        hint = shift_hint(*suggested_shift_s)
    )]
    TrackOutsideRace {
        track_start: DateTime<Utc>,
        track_end: DateTime<Utc>,
        race_start: DateTime<Utc>,
        race_finish: DateTime<Utc>,
        max_offset_s: u32,
        /// Desplazamiento de horas enteras ([`SUGGESTED_SHIFTS_S`]) con el que el track sí se
        /// solaparía, con el convenio de `offset_s` (hora del track = hora de la picada +
        /// desplazamiento). Es solo una pista: no se aplica.
        suggested_shift_s: Option<i64>,
    },
}

/// Resultado de la alineación de un track con las picadas de un corredor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alignment {
    /// Desfase reloj − cronometraje en segundos: `instante en el track = picada + offset_s`.
    pub offset_s: f64,
    /// `false` si no había picadas útiles suficientes y `offset_s` es 0 por defecto.
    pub offset_estimated: bool,
    /// Confianza en el desfase, de 0 (ninguna) a 1. Ver `docs/alineacion.md`.
    pub confidence: f64,
    /// Detalle de la estimación, para depurar y para la interfaz.
    pub quality: AlignmentQuality,
    /// Cobertura del track sobre la carrera, ya con el desfase aplicado.
    pub coverage: Coverage,
    /// Una entrada por picada del resultado, en el mismo orden (`result.punches[i]`).
    pub punches: Vec<AlignedPunch>,
    /// Avisos legibles, en español.
    pub warnings: Vec<AlignmentWarning>,
}

/// Detalle de la estimación del desfase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentQuality {
    /// Picadas con señal en el desfase elegido que han contado en la estimación.
    pub controls_used: usize,
    /// Fracción de esas picadas cuyo desfase propio (máximo local de la señal en ±15 s) está a
    /// 3 s o menos del global.
    pub support: f64,
    /// Separación entre el mejor desfase y la mejor alternativa a más de 10 s, relativa a la
    /// altura del mejor: 0 = empate, 1 = la alternativa no destaca nada sobre la media.
    pub margin: f64,
    /// Mejor desfase alternativo (a más de 10 s del elegido), si lo hay.
    pub runner_up_offset_s: Option<f64>,
}

/// Cobertura del track sobre la ventana salida–meta, en hora del reloj (picadas + desfase).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Coverage {
    pub track_start: DateTime<Utc>,
    pub track_end: DateTime<Utc>,
    /// Salida (o primera picada con hora) más el desfase.
    pub race_start: DateTime<Utc>,
    /// Meta (o última picada con hora) más el desfase.
    pub race_finish: DateTime<Utc>,
    /// Segundos de carrera antes de que empiece el track (0 si lo cubre).
    pub missing_start_s: f64,
    /// Segundos de carrera después de que acabe el track (0 si lo cubre).
    pub missing_end_s: f64,
    /// Huecos del track (más de `max_gap_s` entre puntos) que tocan la carrera.
    pub gaps: Vec<TrackGap>,
}

/// Hueco entre dos puntos consecutivos del track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackGap {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub duration_s: f64,
}

/// Una picada situada en el track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignedPunch {
    pub code: ControlCode,
    /// Instante de la picada en el cronometraje (UTC); `None` si no se registró.
    pub punch_time: Option<DateTime<Utc>>,
    /// Instante equivalente en el reloj: `punch_time + offset_s`.
    pub track_time: Option<DateTime<Utc>>,
    /// Dónde cae `track_time` en el track; `None` si cae fuera.
    pub location: Option<TrackLocation>,
    /// Si la picada ha servido para estimar el desfase y, si no, por qué.
    pub usage: PunchUsage,
    /// Desfase propio de esta picada (máximo local de la señal a ±15 s del global), si tiene
    /// señal. Sirve para ver qué picadas apoyan el desfase global y cuáles no.
    pub local_offset_s: Option<f64>,
}

/// Posición de un instante dentro del track: entre `points[index]` y `points[index + 1]`, a la
/// fracción `fraction` (0 = en `points[index]`). En el último punto, `fraction` es 0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackLocation {
    pub index: usize,
    pub fraction: f64,
    /// El instante cae en un hueco del track (más de `max_gap_s` entre los dos puntos).
    pub in_gap: bool,
}

/// Papel de una picada en la estimación del desfase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PunchUsage {
    /// Ha contado para estimar el desfase.
    Used,
    /// Salida: su hora suele ser la asignada; solo sirve para la ventana de la carrera.
    Start,
    /// Sin hora en el cronometraje: se ignora.
    NoTime,
    /// Su instante está en el track (o a menos de [`EDGE_SNAP_S`] de su principio o su final),
    /// pero demasiado cerca del borde o de un hueco para calcular la señal: se sitúa, pero no
    /// cuenta para el desfase. Es lo normal en la meta si el reloj se para al picarla.
    NearEdge,
    /// Su instante cae fuera del track o en un hueco: no hay señal.
    NoSignal,
}

/// Aviso de la alineación: tipo con sus datos y mensaje legible en español.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentWarning {
    #[serde(flatten)]
    pub kind: WarningKind,
    pub message: String,
}

/// Tipos de aviso. En JSON van aplanados en [`AlignmentWarning`] con la etiqueta `kind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WarningKind {
    /// La salida no tiene hora: la ventana empieza en la primera picada con hora.
    StartWithoutTime,
    /// La meta no tiene hora: la ventana acaba en la última picada con hora.
    FinishWithoutTime,
    /// Picadas sin hora (sin contar salida y meta, que tienen su propio aviso).
    PunchesWithoutTime { count: usize },
    /// El track empieza después de la salida.
    TrackStartsLate { missing_s: f64 },
    /// El track acaba antes de la meta.
    TrackEndsEarly { missing_s: f64 },
    /// Huecos del track durante la carrera.
    TrackGaps {
        count: usize,
        longest_s: f64,
        max_gap_s: f64,
    },
    /// No hay picadas útiles suficientes: se usa desfase 0.
    OffsetNotEstimated { usable: usize, required: usize },
    /// Pocas picadas útiles: la estimación es frágil.
    FewControls { usable: usize },
    /// Desfase grande en valor absoluto.
    LargeOffset { offset_s: f64 },
    /// El desfase está en el borde de la búsqueda: el real puede estar fuera.
    OffsetAtSearchLimit { offset_s: f64, max_offset_s: u32 },
    /// Hay otro desfase casi igual de bueno.
    AmbiguousOffset { offset_s: f64, alternative_s: f64 },
    /// Confianza por debajo del umbral.
    LowConfidence { confidence: f64 },
}

impl WarningKind {
    /// Mensaje legible en español.
    pub fn message(&self) -> String {
        match self {
            Self::StartWithoutTime => "La salida no tiene hora: la carrera se toma desde la \
                                       primera picada con hora."
                .to_string(),
            Self::FinishWithoutTime => "La meta no tiene hora: la carrera se toma hasta la \
                                        última picada con hora."
                .to_string(),
            Self::PunchesWithoutTime { count } => {
                format!("{count} picada(s) sin hora: no se usan para alinear.")
            }
            Self::TrackStartsLate { missing_s } => format!(
                "El track empieza {} s después de la salida: falta el principio de la carrera.",
                decimal(*missing_s, 0)
            ),
            Self::TrackEndsEarly { missing_s } => format!(
                "El track acaba {} s antes de la meta: falta el final de la carrera.",
                decimal(*missing_s, 0)
            ),
            Self::TrackGaps {
                count,
                longest_s,
                max_gap_s,
            } => format!(
                "El track tiene {count} hueco(s) de más de {} s durante la carrera (el mayor, \
                 de {} s): ahí no se sabe por dónde fue el corredor.",
                decimal(*max_gap_s, 0),
                decimal(*longest_s, 0)
            ),
            Self::OffsetNotEstimated { usable, required } => format!(
                "Solo hay {usable} picada(s) útil(es) para estimar el desfase entre el reloj y \
                 el cronometraje (hacen falta {required}): se usa desfase 0."
            ),
            Self::FewControls { usable } => format!(
                "Solo hay {usable} picadas útiles para estimar el desfase: la estimación es \
                 frágil."
            ),
            Self::LargeOffset { offset_s } => format!(
                "Desfase de {} s entre el reloj y el cronometraje: es mucho; comprueba la hora \
                 del reloj y que el FIT y los splits son de la misma carrera.",
                signed(*offset_s)
            ),
            Self::OffsetAtSearchLimit {
                offset_s,
                max_offset_s,
            } => format!(
                "El desfase estimado ({} s) está en el límite de la búsqueda (±{max_offset_s} \
                 s): el real puede ser mayor.",
                signed(*offset_s)
            ),
            Self::AmbiguousOffset {
                offset_s,
                alternative_s,
            } => format!(
                "Hay otro desfase casi igual de bueno ({} s frente a {} s): la alineación es \
                 dudosa.",
                signed(*alternative_s),
                signed(*offset_s)
            ),
            Self::LowConfidence { confidence } => format!(
                "Confianza baja en la alineación ({}): revisa el desfase a mano.",
                decimal(*confidence, 2)
            ),
        }
    }
}

impl From<WarningKind> for AlignmentWarning {
    fn from(kind: WarningKind) -> Self {
        let message = kind.message();
        Self { kind, message }
    }
}

/// Número con coma decimal, como se escribe en español.
fn decimal(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}").replace('.', ",")
}

/// Desfase con signo y un decimal: `+7,0`, `-3,5`.
fn signed(value: f64) -> String {
    format!("{value:+.1}").replace('.', ",")
}

/// Alinea el track del reloj con las picadas de un resultado.
///
/// Falla solo si no hay nada que alinear (track vacío, ninguna picada con hora, track que no
/// se solapa con la carrera ni con el desfase máximo) o si las opciones no son válidas. Todo lo
/// demás (track incompleto, pocas picadas, desfase dudoso) son avisos en el resultado.
pub fn align(
    track: &Track,
    result: &RaceResult,
    options: &AlignmentOptions,
) -> Result<Alignment, AlignmentError> {
    validate(options)?;
    let (first_point, last_point) = match (track.points.first(), track.points.last()) {
        (Some(first), Some(last)) => (first.time, last.time),
        _ => return Err(AlignmentError::EmptyTrack),
    };

    let mut warnings: Vec<AlignmentWarning> = Vec::new();
    let window = race_window(result, &mut warnings)?;
    let origin = window.start;
    let rel = |t: DateTime<Utc>| seconds_between(origin, t);
    let max_offset = f64::from(options.max_offset_s);

    let race_finish_rel = rel(window.finish);
    let overlaps = |shift: f64| {
        rel(last_point) >= shift - max_offset
            && rel(first_point) <= race_finish_rel + shift + max_offset
    };
    if !overlaps(0.0) {
        return Err(AlignmentError::TrackOutsideRace {
            track_start: first_point,
            track_end: last_point,
            race_start: window.start,
            race_finish: window.finish,
            max_offset_s: options.max_offset_s,
            suggested_shift_s: SUGGESTED_SHIFTS_S
                .into_iter()
                .find(|&shift| overlaps(shift as f64)),
        });
    }

    let point_times: Vec<f64> = track.points.iter().map(|p| rel(p.time)).collect();
    let signal = Signal::build(track, &point_times, race_finish_rel, options);

    // Picadas candidatas a estimar el desfase: con hora y, salvo que se pida, sin la salida.
    let candidates: Vec<(usize, f64)> = result
        .punches
        .iter()
        .enumerate()
        .filter(|(_, p)| options.use_start_punch || p.code != START_CODE)
        .filter_map(|(i, p)| p.time.map(|t| (i, rel(t))))
        .collect();
    let (estimate, usable) = match estimate_offset(&signal, &candidates, options) {
        Ok(e) => {
            let usable = e.used.len();
            (Some(e), usable)
        }
        Err(usable) => (None, usable),
    };

    let offset_s = estimate.as_ref().map_or(0.0, |e| e.offset_s);
    let used: Vec<usize> = estimate.as_ref().map_or_else(Vec::new, |e| e.used.clone());
    if estimate.is_none() {
        warnings.push(
            WarningKind::OffsetNotEstimated {
                usable,
                required: options.min_controls,
            }
            .into(),
        );
    }

    // Avisos sobre el desfase.
    let confidence = estimate.as_ref().map_or(0.0, |e| e.confidence);
    if let Some(e) = &estimate {
        if usable < options.few_controls {
            warnings.push(WarningKind::FewControls { usable }.into());
        }
        if offset_s.abs() >= max_offset - SEARCH_LIMIT_MARGIN_S {
            warnings.push(
                WarningKind::OffsetAtSearchLimit {
                    offset_s,
                    max_offset_s: options.max_offset_s,
                }
                .into(),
            );
        } else if offset_s.abs() > options.large_offset_s {
            warnings.push(WarningKind::LargeOffset { offset_s }.into());
        }
        match e.quality.runner_up_offset_s {
            Some(alternative_s) if e.quality.margin < 1.0 - AMBIGUOUS_PEAK_RATIO => warnings.push(
                WarningKind::AmbiguousOffset {
                    offset_s,
                    alternative_s,
                }
                .into(),
            ),
            _ => {}
        }
        if confidence < options.low_confidence {
            warnings.push(WarningKind::LowConfidence { confidence }.into());
        }
    }

    let offset = seconds_to_delta(offset_s);
    let coverage = coverage(track, &window, offset, options, &mut warnings);

    let punches = result
        .punches
        .iter()
        .enumerate()
        .map(|(i, punch)| {
            let track_time = punch.time.map(|t| t + offset);
            let location = track_time.and_then(|t| locate_near_edge(&point_times, rel(t), options));
            let local_offset_s = estimate
                .as_ref()
                .and_then(|e| e.local_offsets.iter().find(|(j, _)| *j == i))
                .map(|(_, local)| *local);
            let usage = if punch.time.is_none() {
                PunchUsage::NoTime
            } else if used.contains(&i) {
                PunchUsage::Used
            } else if punch.code == START_CODE && !options.use_start_punch {
                PunchUsage::Start
            } else if location.is_some_and(|l| !l.in_gap)
                && track_time.is_some_and(|t| signal.at(rel(t)).is_none())
            {
                PunchUsage::NearEdge
            } else {
                PunchUsage::NoSignal
            };
            AlignedPunch {
                code: punch.code,
                punch_time: punch.time,
                track_time,
                location,
                usage,
                local_offset_s,
            }
        })
        .collect();

    Ok(Alignment {
        offset_s,
        offset_estimated: estimate.is_some(),
        confidence,
        quality: estimate.map_or(
            AlignmentQuality {
                controls_used: 0,
                support: 0.0,
                margin: 0.0,
                runner_up_offset_s: None,
            },
            |e| e.quality,
        ),
        coverage,
        punches,
        warnings,
    })
}

fn validate(options: &AlignmentOptions) -> Result<(), AlignmentError> {
    let finite_at_least = |v: f64, min: f64| v.is_finite() && v >= min;
    if options.max_offset_s > MAX_SEARCH_S {
        return Err(AlignmentError::InvalidOptions(
            "max_offset_s no puede pasar de 600 s",
        ));
    }
    if !(1..=MAX_WINDOW_S).contains(&options.turn_window_s) {
        return Err(AlignmentError::InvalidOptions(
            "turn_window_s debe estar entre 1 y 60 s",
        ));
    }
    if options.smoothing_s > MAX_WINDOW_S {
        return Err(AlignmentError::InvalidOptions(
            "smoothing_s no puede pasar de 60 s",
        ));
    }
    if !(options.min_displacement_m.is_finite() && options.min_displacement_m > 0.0) {
        return Err(AlignmentError::InvalidOptions(
            "min_displacement_m debe ser positivo",
        ));
    }
    if !finite_at_least(options.max_gap_s, 1.0) {
        return Err(AlignmentError::InvalidOptions(
            "max_gap_s debe ser al menos 1 s",
        ));
    }
    if !finite_at_least(options.coverage_tolerance_s, 0.0)
        || !finite_at_least(options.large_offset_s, 0.0)
    {
        return Err(AlignmentError::InvalidOptions(
            "coverage_tolerance_s y large_offset_s no pueden ser negativos",
        ));
    }
    if !(0.0..=1.0).contains(&options.low_confidence) {
        return Err(AlignmentError::InvalidOptions(
            "low_confidence debe estar entre 0 y 1",
        ));
    }
    if options.min_controls == 0 {
        return Err(AlignmentError::InvalidOptions(
            "min_controls debe ser al menos 1",
        ));
    }
    Ok(())
}

/// Ventana salida–meta en hora del cronometraje.
struct RaceWindow {
    start: DateTime<Utc>,
    finish: DateTime<Utc>,
}

/// Salida y meta del resultado; si les falta la hora, la primera y la última picada con hora.
fn race_window(
    result: &RaceResult,
    warnings: &mut Vec<AlignmentWarning>,
) -> Result<RaceWindow, AlignmentError> {
    let punches = &result.punches;
    let timed: Vec<DateTime<Utc>> = punches.iter().filter_map(|p| p.time).collect();
    let (Some(&first), Some(&last)) = (timed.first(), timed.last()) else {
        return Err(AlignmentError::NoPunchTimes);
    };

    let start_punch = punches.first().filter(|p| p.code == START_CODE);
    let finish_punch = punches.last().filter(|p| p.code == FINISH_CODE);
    let start = match start_punch.and_then(|p| p.time) {
        Some(t) => t,
        None => {
            warnings.push(WarningKind::StartWithoutTime.into());
            first
        }
    };
    let finish = match finish_punch.and_then(|p| p.time) {
        Some(t) => t,
        None => {
            warnings.push(WarningKind::FinishWithoutTime.into());
            last
        }
    };
    if finish <= start {
        return Err(AlignmentError::NoRaceWindow);
    }

    // Salida y meta sin hora ya tienen su aviso.
    let missing = punches
        .iter()
        .filter(|p| p.time.is_none() && p.code != START_CODE && p.code != FINISH_CODE)
        .count();
    if missing > 0 {
        warnings.push(WarningKind::PunchesWithoutTime { count: missing }.into());
    }
    Ok(RaceWindow { start, finish })
}

/// Cobertura del track sobre la carrera con el desfase aplicado, con sus avisos.
fn coverage(
    track: &Track,
    window: &RaceWindow,
    offset: TimeDelta,
    options: &AlignmentOptions,
    warnings: &mut Vec<AlignmentWarning>,
) -> Coverage {
    let points = &track.points;
    let track_start = points.first().map_or(window.start, |p| p.time);
    let track_end = points.last().map_or(window.finish, |p| p.time);
    let race_start = window.start + offset;
    let race_finish = window.finish + offset;
    let missing_start_s = seconds_between(race_start, track_start).max(0.0);
    let missing_end_s = seconds_between(track_end, race_finish).max(0.0);

    let gaps: Vec<TrackGap> = points
        .windows(2)
        .filter_map(|pair| {
            let (a, b) = (pair[0].time, pair[1].time);
            let duration_s = seconds_between(a, b);
            (duration_s > options.max_gap_s && b > race_start && a < race_finish).then_some(
                TrackGap {
                    start: a,
                    end: b,
                    duration_s,
                },
            )
        })
        .collect();

    if missing_start_s > options.coverage_tolerance_s {
        warnings.push(
            WarningKind::TrackStartsLate {
                missing_s: missing_start_s,
            }
            .into(),
        );
    }
    if missing_end_s > options.coverage_tolerance_s {
        warnings.push(
            WarningKind::TrackEndsEarly {
                missing_s: missing_end_s,
            }
            .into(),
        );
    }
    if !gaps.is_empty() {
        let longest_s = gaps.iter().map(|g| g.duration_s).fold(0.0, f64::max);
        warnings.push(
            WarningKind::TrackGaps {
                count: gaps.len(),
                longest_s,
                max_gap_s: options.max_gap_s,
            }
            .into(),
        );
    }

    Coverage {
        track_start,
        track_end,
        race_start,
        race_finish,
        missing_start_s,
        missing_end_s,
        gaps,
    }
}

/// Segundos de `a` a `b` (positivo si `b` es posterior), con resolución de microsegundos.
fn seconds_between(a: DateTime<Utc>, b: DateTime<Utc>) -> f64 {
    let delta = b - a;
    match delta.num_microseconds() {
        Some(us) => us as f64 / 1e6,
        None => delta.num_milliseconds() as f64 / 1e3,
    }
}

fn seconds_to_delta(seconds: f64) -> TimeDelta {
    TimeDelta::microseconds((seconds * 1e6).round() as i64)
}

/// Sitúa el instante `t` (segundos relativos) entre dos puntos del track.
fn locate(point_times: &[f64], t: f64, options: &AlignmentOptions) -> Option<TrackLocation> {
    let (&first, &last) = (point_times.first()?, point_times.last()?);
    if !(first..=last).contains(&t) {
        return None;
    }
    // Primer punto posterior a `t`; el anterior (o igual) es `index`.
    let index = point_times.partition_point(|&p| p <= t).checked_sub(1)?;
    match point_times.get(index + 1) {
        None => Some(TrackLocation {
            index,
            fraction: 0.0,
            in_gap: false,
        }),
        Some(&next) => {
            let span = next - point_times[index];
            let fraction = if span > 0.0 {
                (t - point_times[index]) / span
            } else {
                0.0
            };
            Some(TrackLocation {
                index,
                fraction,
                in_gap: span > options.max_gap_s,
            })
        }
    }
}

/// Como [`locate`], pero un instante fuera del track por [`EDGE_SNAP_S`] o menos se sitúa en el
/// primer o el último punto (`fraction` 0). Solo para picadas: la señal no se extrapola.
fn locate_near_edge(
    point_times: &[f64],
    t: f64,
    options: &AlignmentOptions,
) -> Option<TrackLocation> {
    if let Some(location) = locate(point_times, t, options) {
        return Some(location);
    }
    let (&first, &last) = (point_times.first()?, point_times.last()?);
    let index = if t < first && first - t <= EDGE_SNAP_S {
        0
    } else if t > last && t - last <= EDGE_SNAP_S {
        point_times.len() - 1
    } else {
        return None;
    };
    Some(TrackLocation {
        index,
        fraction: 0.0,
        in_gap: false,
    })
}

/// Señal de "baliza" remuestreada a 1 Hz sobre la parte útil del track.
struct Signal {
    /// Instante (segundos relativos a la salida) del primer valor.
    t0: f64,
    /// Señal suavizada, menos su media en la carrera; `None` donde no hay track.
    values: Vec<Option<f64>>,
}

impl Signal {
    fn build(
        track: &Track,
        point_times: &[f64],
        race_finish_rel: f64,
        options: &AlignmentOptions,
    ) -> Signal {
        let empty = Signal {
            t0: 0.0,
            values: Vec::new(),
        };
        let (Some(&first), Some(&last)) = (point_times.first(), point_times.last()) else {
            return empty;
        };
        let max_offset = f64::from(options.max_offset_s);
        let pad = f64::from(options.turn_window_s)
            + f64::from(options.smoothing_s)
            + LOCAL_WINDOW_S as f64
            + 2.0;
        let lo = first.max(-max_offset - pad);
        let hi = last.min(race_finish_rel + max_offset + pad);
        if hi < lo {
            return empty;
        }
        let n = (hi - lo).floor() as usize + 1;
        let lat0 = track
            .points
            .first()
            .map_or(0.0, |p| p.lat.to_radians())
            .cos();

        // Remuestreo: posición en metros (plano local) y cadencia, por interpolación lineal.
        let mut xy: Vec<Option<(f64, f64)>> = Vec::with_capacity(n);
        let mut cadence: Vec<Option<f64>> = Vec::with_capacity(n);
        for k in 0..n {
            let t = lo + k as f64;
            let Some(loc) = locate(point_times, t, options).filter(|l| !l.in_gap) else {
                xy.push(None);
                cadence.push(None);
                continue;
            };
            let a = &track.points[loc.index];
            let b = track.points.get(loc.index + 1).unwrap_or(a);
            let f = loc.fraction;
            let lerp = |u: f64, v: f64| u + f * (v - u);
            let lat = lerp(a.lat, b.lat);
            let lon = lerp(a.lon, b.lon);
            xy.push(Some((
                EARTH_RADIUS_M * lon.to_radians() * lat0,
                EARTH_RADIUS_M * lat.to_radians(),
            )));
            cadence.push(match (a.cadence_spm, b.cadence_spm) {
                (Some(u), Some(v)) => Some(lerp(u, v)),
                (Some(u), None) if f == 0.0 => Some(u),
                _ => None,
            });
        }

        let dist = |p: (f64, f64), q: (f64, f64)| (q.0 - p.0).hypot(q.1 - p.1);
        let at = |k: isize| -> Option<(f64, f64)> {
            usize::try_from(k)
                .ok()
                .and_then(|k| xy.get(k).copied().flatten())
        };
        let speed: Vec<Option<f64>> = (0..n as isize)
            .map(|k| Some(dist(at(k - 1)?, at(k + 1)?) / 2.0))
            .collect();

        // Referencias del corredor en esta carrera: medianas en la ventana salida–meta.
        let in_race = |k: usize| {
            let t = lo + k as f64;
            (0.0..=race_finish_rel).contains(&t)
        };
        let race_len = (0..n).filter(|&k| in_race(k)).count();
        let speed_ref = median((0..n).filter(|&k| in_race(k)).filter_map(|k| speed[k]))
            .filter(|&v| v >= MIN_REFERENCE_SPEED_MPS);
        let race_cadence: Vec<f64> = (0..n)
            .filter(|&k| in_race(k))
            .filter_map(|k| cadence[k])
            .collect();
        let cadence_ref = if (race_cadence.len() as f64) >= MIN_CADENCE_COVERAGE * race_len as f64 {
            median(race_cadence.into_iter()).filter(|&c| c > 0.0)
        } else {
            None
        };

        let w = options.turn_window_s as isize;
        let raw: Vec<Option<f64>> = (0..n as isize)
            .map(|k| {
                // El giro es la señal principal: sin él (bordes del track, huecos) no hay
                // valor, para no mezclar escalas.
                let (before, here, after) = (at(k - w)?, at(k)?, at(k + w)?);
                let (ax, ay) = (here.0 - before.0, here.1 - before.1);
                let (bx, by) = (after.0 - here.0, after.1 - here.1);
                let (la, lb) = (ax.hypot(ay), bx.hypot(by));
                // Con el corredor casi parado el rumbo es ruido del GPS: el giro pesa menos (la
                // bajada de velocidad ya lo recoge).
                let reliability = (la.min(lb) / options.min_displacement_m).min(1.0);
                let turn = if la > 0.0 && lb > 0.0 {
                    let cos = ((ax * bx + ay * by) / (la * lb)).clamp(-1.0, 1.0);
                    (1.0 - cos) / 2.0
                } else {
                    0.0
                };
                let mut total = TURN_WEIGHT * turn * reliability;
                let mut weight = TURN_WEIGHT;
                if let (Some(v), Some(v_ref)) = (speed[k as usize], speed_ref) {
                    total += SPEED_WEIGHT * (1.0 - v / v_ref).clamp(0.0, 1.0);
                    weight += SPEED_WEIGHT;
                }
                if let (Some(c), Some(c_ref)) = (cadence[k as usize], cadence_ref) {
                    total += CADENCE_WEIGHT * (1.0 - c / c_ref).clamp(0.0, 1.0);
                    weight += CADENCE_WEIGHT;
                }
                Some(total / weight)
            })
            .collect();

        // Suavizado triangular de semiancho `smoothing_s`.
        let h = options.smoothing_s as isize;
        let smoothed: Vec<Option<f64>> = (0..n as isize)
            .map(|k| {
                raw[k as usize]?;
                let mut total = 0.0;
                let mut weight = 0.0;
                for j in -h..=h {
                    let Some(Some(v)) = usize::try_from(k + j).ok().and_then(|i| raw.get(i)) else {
                        continue;
                    };
                    let kernel = (h + 1 - j.abs()) as f64;
                    total += kernel * v;
                    weight += kernel;
                }
                Some(total / weight)
            })
            .collect();

        // Se resta la media en la carrera: un instante cualquiera vale 0 de media, así una
        // picada sin señal (fuera del track) cuenta como 0 y no favorece ningún desfase.
        let race_values: Vec<f64> = (0..n)
            .filter(|&k| in_race(k))
            .filter_map(|k| smoothed[k])
            .collect();
        let Some(baseline) = mean(&race_values) else {
            return empty;
        };
        Signal {
            t0: lo,
            values: smoothed
                .into_iter()
                .map(|v| v.map(|v| v - baseline))
                .collect(),
        }
    }

    /// Valor de la señal en `t` (segundos relativos), interpolado; `None` sin track.
    fn at(&self, t: f64) -> Option<f64> {
        let k = t - self.t0;
        if !k.is_finite() || k < 0.0 {
            return None;
        }
        let k0 = k.floor() as usize;
        let f = k - k0 as f64;
        let a = self.values.get(k0).copied().flatten()?;
        if f == 0.0 {
            return Some(a);
        }
        let b = self.values.get(k0 + 1).copied().flatten()?;
        Some(a + f * (b - a))
    }
}

/// Mediana de los valores finitos; `None` si no hay ninguno.
pub(crate) fn median(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut v: Vec<f64> = values.filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let mid = v.len() / 2;
    Some(if v.len() % 2 == 0 {
        (v[mid - 1] + v[mid]) / 2.0
    } else {
        v[mid]
    })
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

/// Resultado de la búsqueda del desfase.
struct Estimate {
    offset_s: f64,
    confidence: f64,
    quality: AlignmentQuality,
    /// Posiciones en `result.punches` de las picadas con señal en el desfase elegido.
    used: Vec<usize>,
    /// Desfase propio de cada picada usada.
    local_offsets: Vec<(usize, f64)>,
}

/// Busca el desfase que maximiza la señal media en las picadas. Si no hay picadas útiles
/// suficientes, devuelve cuántas había.
fn estimate_offset(
    signal: &Signal,
    candidates: &[(usize, f64)],
    options: &AlignmentOptions,
) -> Result<Estimate, usize> {
    if candidates.is_empty() {
        return Err(0);
    }
    let m = i64::from(options.max_offset_s);
    let n = candidates.len() as f64;
    let score = |d: i64| -> f64 {
        candidates
            .iter()
            .filter_map(|&(_, t)| signal.at(t + d as f64))
            .sum::<f64>()
            / n
    };
    let offsets: Vec<i64> = (-m..=m).collect();
    let scores: Vec<f64> = offsets.iter().map(|&d| score(d)).collect();

    // Mejor desfase de la rejilla; en caso de empate, el más cercano a 0.
    let mut best = 0usize;
    for (i, (&d, &s)) in offsets.iter().zip(&scores).enumerate() {
        let (bd, bs) = (offsets[best], scores[best]);
        if s > bs || (s == bs && d.abs() < bd.abs()) {
            best = i;
        }
    }
    let best_d = offsets[best];
    let used: Vec<usize> = candidates
        .iter()
        .filter(|&&(_, t)| signal.at(t + best_d as f64).is_some())
        .map(|&(i, _)| i)
        .collect();
    if used.len() < options.min_controls {
        return Err(used.len());
    }

    // Refinamiento: vértice de la parábola por el mejor punto y sus vecinos.
    let mut offset_s = best_d as f64;
    if best > 0 && best + 1 < scores.len() {
        let (y0, y1, y2) = (scores[best - 1], scores[best], scores[best + 1]);
        let curvature = y0 - 2.0 * y1 + y2;
        if curvature < 0.0 {
            offset_s += (0.5 * (y0 - y2) / curvature).clamp(-0.5, 0.5);
        }
    }

    // Separación con la mejor alternativa fuera del pico.
    let average = mean(&scores).unwrap_or(0.0);
    let peak = scores[best] - average;
    let runner_up = offsets
        .iter()
        .zip(&scores)
        .filter(|&(&d, _)| (d - best_d).abs() > SECOND_PEAK_EXCLUSION_S)
        .max_by(|a, b| a.1.total_cmp(b.1));
    let margin = match runner_up {
        Some((_, &s)) if peak > 0.0 => ((scores[best] - s) / peak).clamp(0.0, 1.0),
        Some(_) => 0.0,
        None => 1.0,
    };

    // Desfase propio de cada picada: máximo local de su señal a ±15 s del global.
    let local_offsets: Vec<(usize, f64)> = candidates
        .iter()
        .filter(|(i, _)| used.contains(i))
        .filter_map(|&(i, t)| {
            let mut local: Option<(i64, f64)> = None;
            for d in best_d - LOCAL_WINDOW_S..=best_d + LOCAL_WINDOW_S {
                let Some(v) = signal.at(t + d as f64) else {
                    continue;
                };
                let better = match local {
                    None => true,
                    Some((ld, lv)) => {
                        v > lv || (v == lv && (d - best_d).abs() < (ld - best_d).abs())
                    }
                };
                if better {
                    local = Some((d, v));
                }
            }
            local.map(|(d, _)| (i, d as f64))
        })
        .collect();
    let supporting = local_offsets
        .iter()
        .filter(|(_, d)| (d - best_d as f64).abs() <= SUPPORT_TOLERANCE_S as f64)
        .count();
    let support = supporting as f64 / used.len() as f64;

    let confidence = confidence(support, margin, used.len(), options);
    Ok(Estimate {
        offset_s,
        confidence,
        quality: AlignmentQuality {
            controls_used: used.len(),
            support,
            margin,
            runner_up_offset_s: runner_up.map(|(&d, _)| d as f64),
        },
        used,
        local_offsets,
    })
}

/// Confianza 0–1 a partir del apoyo de las picadas, la separación del pico y el número de
/// picadas (ver `docs/alineacion.md`).
fn confidence(support: f64, margin: f64, used: usize, options: &AlignmentOptions) -> f64 {
    let support_term = ((support - SUPPORT_FLOOR) / (SUPPORT_FULL - SUPPORT_FLOOR)).clamp(0.0, 1.0);
    let margin_term = (margin / MARGIN_FULL).clamp(0.0, 1.0);
    let count_term = (used as f64 / options.few_controls.max(1) as f64).min(1.0);
    support_term * margin_term * count_term
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_use_decimal_comma() {
        assert_eq!(decimal(0.456, 2), "0,46");
        assert_eq!(decimal(300.4, 0), "300");
        assert_eq!(signed(7.14), "+7,1");
        assert_eq!(signed(-3.04), "-3,0");
    }

    #[test]
    fn locate_finds_the_surrounding_points() {
        let options = AlignmentOptions::default();
        let times = [0.0, 1.0, 2.0, 30.0];
        assert_eq!(locate(&times, -0.5, &options), None);
        assert_eq!(locate(&times, 30.5, &options), None);
        let at = |t| locate(&times, t, &options).unwrap();
        assert_eq!((at(0.0).index, at(0.0).fraction), (0, 0.0));
        assert_eq!((at(1.25).index, at(1.25).fraction), (1, 0.25));
        // Entre 2 y 30 hay un hueco de 28 s (> 10 s).
        assert_eq!(
            (at(9.0).index, at(9.0).fraction, at(9.0).in_gap),
            (2, 0.25, true)
        );
        assert_eq!((at(30.0).index, at(30.0).fraction), (3, 0.0));
    }

    #[test]
    fn punches_just_outside_the_track_snap_to_its_ends() {
        let options = AlignmentOptions::default();
        let times = [0.0, 1.0, 2.0];
        let near = |t| locate_near_edge(&times, t, &options).map(|l| (l.index, l.fraction));
        // Dentro, igual que `locate`.
        assert_eq!(near(1.5), Some((1, 0.5)));
        // Fuera por 2 s o menos: el punto del extremo.
        assert_eq!(near(-2.0), Some((0, 0.0)));
        assert_eq!(near(2.1), Some((2, 0.0)));
        assert_eq!(near(4.0), Some((2, 0.0)));
        // Más lejos, nada.
        assert_eq!(near(-2.1), None);
        assert_eq!(near(4.1), None);
        assert_eq!(locate_near_edge(&[], 0.0, &options), None);
    }

    #[test]
    fn median_and_mean() {
        assert_eq!(median([3.0, 1.0, 2.0].into_iter()), Some(2.0));
        assert_eq!(median([4.0, 1.0, 2.0, 3.0].into_iter()), Some(2.5));
        assert_eq!(median(std::iter::empty()), None);
        assert_eq!(mean(&[1.0, 2.0]), Some(1.5));
        assert_eq!(mean(&[]), None);
    }

    #[test]
    fn confidence_terms() {
        let options = AlignmentOptions::default();
        // Apoyo y separación de sobra con 5 picadas: 1.
        assert_eq!(confidence(0.9, 0.8, 5, &options), 1.0);
        // Apoyo de azar: 0.
        assert_eq!(confidence(0.4, 1.0, 20, &options), 0.0);
        // Mitad de apoyo (0,6), separación completa, 3 de 5 picadas: 0,5 × 1 × 0,6.
        assert!((confidence(0.6, 0.6, 3, &options) - 0.3).abs() < 1e-12);
    }

    #[test]
    fn every_warning_has_a_message() {
        let kinds = [
            WarningKind::StartWithoutTime,
            WarningKind::FinishWithoutTime,
            WarningKind::PunchesWithoutTime { count: 2 },
            WarningKind::TrackStartsLate { missing_s: 30.0 },
            WarningKind::TrackEndsEarly { missing_s: 12.4 },
            WarningKind::TrackGaps {
                count: 1,
                longest_s: 41.0,
                max_gap_s: 10.0,
            },
            WarningKind::OffsetNotEstimated {
                usable: 1,
                required: 3,
            },
            WarningKind::FewControls { usable: 4 },
            WarningKind::LargeOffset { offset_s: -35.0 },
            WarningKind::OffsetAtSearchLimit {
                offset_s: 60.0,
                max_offset_s: 60,
            },
            WarningKind::AmbiguousOffset {
                offset_s: 3.0,
                alternative_s: 25.0,
            },
            WarningKind::LowConfidence { confidence: 0.25 },
        ];
        for kind in kinds {
            let warning = AlignmentWarning::from(kind.clone());
            assert!(!warning.message.is_empty());
            assert_eq!(warning.message, kind.message());
        }
        assert_eq!(
            WarningKind::TrackEndsEarly { missing_s: 12.4 }.message(),
            "El track acaba 12 s antes de la meta: falta el final de la carrera."
        );
        assert_eq!(
            WarningKind::LargeOffset { offset_s: -35.0 }.message(),
            "Desfase de -35,0 s entre el reloj y el cronometraje: es mucho; comprueba la hora \
             del reloj y que el FIT y los splits son de la misma carrera."
        );
    }
}
