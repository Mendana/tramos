//! Resumen en frases (#126): unas pocas frases en español llano, arriba del histórico y de cada
//! carrera, para que quien no es de datos vea lo esencial sin abrir ningún análisis.
//!
//! Las frases salen de reglas sencillas **sobre los resultados de los análisis que ya existen**
//! (P1, P2, P5, P6, P7, P8, P9, P11 y P13): aquí no se recalcula nada, solo se comparan sus
//! números con un umbral. Cada regla tiene:
//!
//! - un **umbral** para salir;
//! - un **mínimo de datos** por debajo del cual no sale;
//! - un **mínimo suficiente** por debajo del cual sale marcada como «pocos datos», con un aviso;
//! - una **prioridad**, para quedarse con las [`MAX_INSIGHTS`] más importantes;
//! - un **destino**: la vista que la justifica, para el enlace.
//!
//! Las reglas, sus umbrales y sus textos están en `docs/frases.md`.

use serde::{Deserialize, Serialize};

use crate::after_error::AfterError;
use crate::common_errors::CommonErrors;
use crate::days_off::DaysOff;
use crate::history::{History, race_third};
use crate::leg_length::LegLengthStats;
use crate::loss_breakdown::BreakdownHistory;
use crate::race_format::RaceFormat;
use crate::runner_report::RunnerReport;
use crate::slope::{SlopeClass, SlopeHistory, SlopeStats};
use crate::taxonomy::Taxonomy;

/// Frases como mucho en cada sitio (decisión de #126).
pub const MAX_INSIGHTS: usize = 3;

/// Carreras a partir de las cuales una frase del histórico deja de llevar el aviso «con pocas
/// carreras».
pub const SUFFICIENT_RACES: usize = 5;
/// Tramos mínimos de un grupo (cubo de duración, clase de desnivel, tramos tras un error) para
/// que una frase pueda salir.
pub const MIN_LEGS: usize = 5;
/// Tramos a partir de los cuales la frase deja de llevar el aviso «con pocos tramos». Coincide
/// con el mínimo de la vista de grupo (`group::MIN_BUCKET_LEGS`).
pub const SUFFICIENT_LEGS: usize = 10;
/// Diferencia mínima entre dos tasas de error para decir que una es peor (10 puntos)...
pub const RATE_GAP: f64 = 0.10;
/// ... y, además, cuántas veces la otra como mínimo (1,5): 12 % frente a 2 % sí; 45 % frente a
/// 35 %, no.
pub const RATE_RATIO: f64 = 1.5;
/// Diferencia mínima de IR para decir que se rinde peor (10 puntos).
pub const PERFORMANCE_GAP: f64 = 0.10;

/// Formatos (P6): carreras mínimas de cada formato que se compara.
pub const MIN_FORMAT_RACES: usize = 2;
/// Errores más comunes (P9): errores con tipo mínimos, los suficientes y la parte mínima del
/// tipo más común.
pub const MIN_TYPED_ERRORS: usize = 3;
pub const SUFFICIENT_TYPED_ERRORS: usize = 10;
pub const TOP_TYPE_SHARE: f64 = 0.4;
/// ¿Lento o desorientado? (P2): errores repartidos mínimos, los suficientes y la parte mínima
/// de la pérdida que tiene que ser de una sola causa.
pub const MIN_BREAKDOWN_ERRORS: usize = 3;
pub const SUFFICIENT_BREAKDOWN_ERRORS: usize = 10;
pub const BREAKDOWN_SHARE: f64 = 0.5;
/// Días sin competir (P11): carreras mínimas del cubo (y [`MIN_LEGS`] tramos de entrada).
pub const MIN_DAYS_OFF_RACES: usize = 2;

/// Carrera: tramos con pérdida mínimos para que salga alguna frase, y los suficientes para que
/// no lleve el aviso «con pocos tramos».
pub const MIN_RACE_LEGS: usize = 3;
pub const SUFFICIENT_RACE_LEGS: usize = 8;
/// Pérdida concentrada (P1): parte del tiempo perdido que se llevan uno o dos tramos.
pub const CONCENTRATED_SHARE: f64 = 0.5;
/// Racha perdiendo (P5): tramos y segundos mínimos.
pub const STREAK_LEGS: usize = 3;
pub const STREAK_LOSS_S: f64 = 30.0;
/// Errores por tercio: errores mínimos y parte mínima en un mismo tercio (dos tercios, como
/// fracción `(numerador, denominador)` para comparar sin redondeos).
pub const THIRD_MIN_ERRORS: usize = 3;
pub const THIRD_SHARE: (usize, usize) = (2, 3);

/// Aviso de una frase del histórico con pocas carreras (decisión de #126).
pub const FEW_RACES: &str = "con pocas carreras";
/// Aviso de una frase apoyada en pocos tramos.
pub const FEW_LEGS: &str = "con pocos tramos";
/// Aviso de una frase apoyada en pocos errores.
pub const FEW_ERRORS: &str = "con pocos errores";
/// Aviso de una frase de una carrera con referencia débil (pocos clasificados).
pub const WEAK_REFERENCE: &str = "referencia débil";

/// Qué regla ha dado la frase. También fija su prioridad: ver [`InsightRule::priority`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightRule {
    // Histórico.
    /// P7: un cubo de duración con mucha más tasa de error que la media.
    LegLength,
    /// P9: un tipo de error se lleva buena parte de los errores con tipo.
    CommonError,
    /// P8: tras un error se falla bastante más que tras un tramo limpio.
    AfterError,
    /// P2: la pérdida de los errores es sobre todo desvío, paradas o ritmo.
    LossBreakdown,
    /// P13: en subida (o bajada) se rinde bastante peor que en llano.
    Slope,
    /// P6: un formato con bastante más tasa de error que otro.
    Format,
    /// P11: tras muchos días sin competir se entra peor en mapa.
    DaysOff,
    // Carrera.
    /// P1: ningún error.
    CleanRace,
    /// P1: uno o dos tramos se llevan más de la mitad del tiempo perdido.
    ConcentratedLoss,
    /// P5: una racha larga de tramos perdiendo.
    LosingStreak,
    /// P1: la mayoría de los errores, en un mismo tercio de la carrera.
    ErrorsByThird,
}

impl InsightRule {
    /// Prioridad: menor = más importante. Entre frases con datos suficientes gana la de menor
    /// prioridad; las de pocos datos van siempre detrás.
    pub const fn priority(self) -> u8 {
        match self {
            InsightRule::LegLength => 1,
            InsightRule::CommonError => 2,
            InsightRule::AfterError => 3,
            InsightRule::LossBreakdown => 4,
            InsightRule::Slope => 5,
            InsightRule::Format => 6,
            InsightRule::DaysOff => 7,
            InsightRule::CleanRace => 1,
            InsightRule::ConcentratedLoss => 2,
            InsightRule::LosingStreak => 3,
            InsightRule::ErrorsByThird => 4,
        }
    }
}

/// La vista que justifica una frase: el análisis (la pregunta de `docs/preguntas.md`) que la
/// enseña entera. La app sabe en qué pestaña está cada uno.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightTarget {
    // Histórico.
    /// P6: la tabla y las gráficas por formato.
    Formats,
    /// P7: pérdida según duración del tramo.
    LegLength,
    /// P9: errores más comunes.
    CommonErrors,
    /// P13: por desnivel.
    Slope,
    /// P2 en el histórico: ¿lento o desorientado?
    LossBreakdown,
    /// P8: después de fallar.
    AfterError,
    /// P11: días sin competir.
    DaysOff,
    // Carrera.
    /// P1: la tabla de tramos.
    Legs,
    /// P5: dónde gano y dónde pierdo.
    GainLoss,
}

/// Una frase del resumen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Insight {
    pub rule: InsightRule,
    /// La frase, en español llano y con sus números.
    pub text: String,
    /// Sale con pocos datos: hay que leerla con cuidado.
    pub few_data: bool,
    /// El aviso que va junto a la frase cuando `few_data` («con pocas carreras»…); `None` si no.
    pub caveat: Option<String>,
    pub target: InsightTarget,
}

impl Insight {
    fn new(rule: InsightRule, target: InsightTarget, text: String, caveat: Option<&str>) -> Self {
        Insight {
            rule,
            text,
            few_data: caveat.is_some(),
            caveat: caveat.map(str::to_string),
            target,
        }
    }
}

/// Lo que hace falta para las frases del histórico: los resultados de sus análisis, todos con el
/// mismo filtro.
#[derive(Debug, Clone, Copy)]
pub struct HistoryAnalyses<'a> {
    pub history: &'a History,
    pub by_leg_length: &'a [LegLengthStats],
    pub by_slope: &'a SlopeHistory,
    pub common_errors: &'a CommonErrors,
    pub after_error: &'a AfterError,
    pub days_off: &'a DaysOff,
    pub loss_breakdown: &'a BreakdownHistory,
    /// Para el nombre de los tipos de error; sin ella, sale la clave.
    pub taxonomy: Option<&'a Taxonomy>,
}

/// Las frases del histórico: como mucho [`MAX_INSIGHTS`], las más importantes.
pub fn history_insights(a: &HistoryAnalyses<'_>) -> Vec<Insight> {
    let races = a.history.total.races;
    select(
        [
            leg_length_insight(a.by_leg_length, a.history),
            common_error_insight(a.common_errors, races, a.taxonomy),
            after_error_insight(a.after_error, races),
            loss_breakdown_insight(a.loss_breakdown),
            slope_insight(a.by_slope),
            format_insight(a.history),
            days_off_insight(a.days_off, a.history),
        ]
        .into_iter()
        .flatten()
        .collect(),
    )
}

/// Las frases de una carrera: como mucho [`MAX_INSIGHTS`], las más importantes. Ninguna si la
/// carrera no tiene rendimiento habitual o tiene menos de [`MIN_RACE_LEGS`] tramos con pérdida.
pub fn race_insights(report: &RunnerReport) -> Vec<Insight> {
    let lost = &report.lost_time;
    let with_loss = lost.legs.iter().filter(|l| l.loss_s.is_some()).count();
    if lost.usual_performance.is_none() || with_loss < MIN_RACE_LEGS {
        return Vec::new();
    }
    let caveat = if report.course.weak_reference {
        Some(WEAK_REFERENCE)
    } else if with_loss < SUFFICIENT_RACE_LEGS {
        Some(FEW_LEGS)
    } else {
        None
    };
    select(
        [
            clean_race_insight(report, caveat),
            concentrated_loss_insight(report, caveat),
            losing_streak_insight(report, caveat),
            errors_by_third_insight(report, caveat),
        ]
        .into_iter()
        .flatten()
        .collect(),
    )
}

/// Primero las de datos suficientes; dentro, por prioridad. Se queda con [`MAX_INSIGHTS`].
fn select(mut insights: Vec<Insight>) -> Vec<Insight> {
    insights.sort_by_key(|i| (i.few_data, i.rule.priority()));
    insights.truncate(MAX_INSIGHTS);
    insights
}

/// `a` es claramente mayor que `b`: al menos [`RATE_GAP`] más y [`RATE_RATIO`] veces.
fn clearly_higher(a: f64, b: f64) -> bool {
    at_least(a - b, RATE_GAP) && at_least(a, RATE_RATIO * b)
}

/// `value ≥ threshold`, sin que el redondeo de los decimales deje fuera el caso justo
/// (`0,25 − 0,15` da `0,0999…`): los umbrales **entran**.
fn at_least(value: f64, threshold: f64) -> bool {
    value >= threshold - 1e-9
}

/// Aviso según las carreras y los casos (tramos o errores) en los que se apoya una frase del
/// histórico. Primero las carreras: es el aviso que pidió el usuario.
fn history_caveat(races: usize, cases: usize, sufficient: usize, few: &str) -> Option<&str> {
    if races < SUFFICIENT_RACES {
        Some(FEW_RACES)
    } else if cases < sufficient {
        Some(few)
    } else {
        None
    }
}

// --- Histórico --------------------------------------------------------------------------------

/// P7: el cubo de duración con más tasa de error, entre los de al menos [`MIN_LEGS`] tramos,
/// frente a la tasa de error del total.
fn leg_length_insight(buckets: &[LegLengthStats], history: &History) -> Option<Insight> {
    let total_rate = history.total.error_rate?;
    let (bucket, rate) = buckets
        .iter()
        .filter(|b| b.legs >= MIN_LEGS)
        .filter_map(|b| Some((b, b.error_rate?)))
        // A igual tasa, el primero (el más corto).
        .fold(
            None,
            |worst: Option<(&LegLengthStats, f64)>, (b, r)| match worst {
                Some((_, w)) if w >= r => worst,
                _ => Some((b, r)),
            },
        )?;
    if !clearly_higher(rate, total_rate) {
        return None;
    }
    let text = format!(
        "Fallas más en los tramos {} ({}): el {} son error, frente al {} de media.",
        length_word(bucket),
        length_label(bucket),
        pct(rate),
        pct(total_rate)
    );
    Some(Insight::new(
        InsightRule::LegLength,
        InsightTarget::LegLength,
        text,
        history_caveat(history.total.races, bucket.legs, SUFFICIENT_LEGS, FEW_LEGS),
    ))
}

/// «cortos» hasta 1 min, «largos» desde 4 min y «medios» en medio.
fn length_word(b: &LegLengthStats) -> &'static str {
    match b.to_s {
        Some(to) if to <= 60.0 => "cortos",
        _ if b.from_s >= 240.0 => "largos",
        _ => "medios",
    }
}

/// «de 20 a 30 s», «de 30 s a 1 min», «de 4 a 8 min», «de 8 min o más».
fn length_label(b: &LegLengthStats) -> String {
    let unit = |s: f64| {
        if s >= 60.0 {
            (format!("{:.0}", s / 60.0), "min")
        } else {
            (format!("{s:.0}"), "s")
        }
    };
    let (from, from_unit) = unit(b.from_s);
    match b.to_s {
        None => format!("de {from} {from_unit} o más"),
        Some(to) => {
            let (to, to_unit) = unit(to);
            if from_unit == to_unit {
                format!("de {from} a {to} {to_unit}")
            } else {
                format!("de {from} {from_unit} a {to} {to_unit}")
            }
        }
    }
}

/// P9: el tipo de error más común, si se lleva al menos [`TOP_TYPE_SHARE`] de los errores con
/// tipo y son al menos [`MIN_TYPED_ERRORS`].
fn common_error_insight(
    errors: &CommonErrors,
    races: usize,
    taxonomy: Option<&Taxonomy>,
) -> Option<Insight> {
    let total = &errors.total;
    let typed = total.errors.saturating_sub(total.untyped);
    // `by_type` va de más a menos errores.
    let top = total.by_type.first()?;
    if typed < MIN_TYPED_ERRORS || top.errors == 0 {
        return None;
    }
    let share = top.errors as f64 / typed as f64;
    if !at_least(share, TOP_TYPE_SHARE) {
        return None;
    }
    let label = taxonomy
        .and_then(|t| t.error_type(&top.error_type))
        .map_or(top.error_type.as_str(), |t| t.label.as_str());
    let text = format!(
        "Tu error más común: {} ({} de tus {} errores con tipo).",
        lowercase_first(label),
        top.errors,
        typed
    );
    Some(Insight::new(
        InsightRule::CommonError,
        InsightTarget::CommonErrors,
        text,
        history_caveat(races, typed, SUFFICIENT_TYPED_ERRORS, FEW_ERRORS),
    ))
}

fn lowercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// P8: tasa de error tras un error frente a tras un tramo limpio.
fn after_error_insight(after: &AfterError, races: usize) -> Option<Insight> {
    let (ae, ac) = (&after.after_error, &after.after_clean);
    if ae.legs < MIN_LEGS || ac.legs < MIN_LEGS {
        return None;
    }
    let (ae_rate, ac_rate) = (ae.error_rate?, ac.error_rate?);
    if !clearly_higher(ae_rate, ac_rate) {
        return None;
    }
    let text = format!(
        "Un error suele traer otro: tras fallar, fallas el {} de los tramos; tras un tramo \
         limpio, el {}.",
        pct(ae_rate),
        pct(ac_rate)
    );
    Some(Insight::new(
        InsightRule::AfterError,
        InsightTarget::AfterError,
        text,
        history_caveat(races, ae.legs, SUFFICIENT_LEGS, FEW_ERRORS),
    ))
}

/// P2: si una de las tres partes (desvío, paradas o ritmo) se lleva al menos
/// [`BREAKDOWN_SHARE`] de la pérdida de los errores repartidos.
fn loss_breakdown_insight(b: &BreakdownHistory) -> Option<Insight> {
    let e = &b.errors;
    if e.legs < MIN_BREAKDOWN_ERRORS || e.loss_s <= 0.0 {
        return None;
    }
    let (part, seconds) = [
        ("detour", e.detour_s),
        ("stopped", e.stopped_s),
        ("pace", e.pace_s),
    ]
    .into_iter()
    .max_by(|a, b| a.1.total_cmp(&b.1))?;
    let share = seconds / e.loss_s;
    if !at_least(share, BREAKDOWN_SHARE) {
        return None;
    }
    let text = match part {
        "detour" => format!(
            "Cuando fallas, sobre todo te desvías: el {} del tiempo de tus errores es por \
             correr de más.",
            pct(share)
        ),
        "stopped" => format!(
            "Cuando fallas, sobre todo te paras: el {} del tiempo de tus errores es tiempo \
             parado.",
            pct(share)
        ),
        _ => format!(
            "Cuando fallas, sobre todo vas más lento: el {} del tiempo de tus errores es ritmo, \
             no desvío ni paradas.",
            pct(share)
        ),
    };
    Some(Insight::new(
        InsightRule::LossBreakdown,
        InsightTarget::LossBreakdown,
        text,
        history_caveat(
            b.races_with_track,
            e.legs,
            SUFFICIENT_BREAKDOWN_ERRORS,
            FEW_ERRORS,
        ),
    ))
}

/// P13: la clase (subida o bajada) con menos IR, frente al llano.
fn slope_insight(s: &SlopeHistory) -> Option<Insight> {
    let class = |c: SlopeClass| s.by_class.iter().find(|x| x.class == c);
    let usable = |x: &&SlopeStats| x.legs >= MIN_LEGS && x.mean_performance.is_some();
    let flat = class(SlopeClass::Flat).filter(usable)?;
    let flat_ir = flat.mean_performance?;
    let (worst, worst_ir) = [SlopeClass::Uphill, SlopeClass::Downhill]
        .into_iter()
        .filter_map(class)
        .filter(usable)
        .filter_map(|x| Some((x, x.mean_performance?)))
        // A igual IR, la subida (va primero).
        .fold(None, |w: Option<(&SlopeStats, f64)>, (x, ir)| match w {
            Some((_, wir)) if wir <= ir => w,
            _ => Some((x, ir)),
        })?;
    if !at_least(flat_ir - worst_ir, PERFORMANCE_GAP) {
        return None;
    }
    let (name, place) = match worst.class {
        SlopeClass::Downhill => ("Las bajadas", "en bajada"),
        _ => ("Las subidas", "en subida"),
    };
    let text = format!(
        "{name} te frenan: rindes al {} {place} y al {} en llano.",
        pct(worst_ir),
        pct(flat_ir)
    );
    let legs = worst.legs.min(flat.legs);
    Some(Insight::new(
        InsightRule::Slope,
        InsightTarget::Slope,
        text,
        history_caveat(s.races_with_track, legs, SUFFICIENT_LEGS, FEW_LEGS),
    ))
}

/// P6: el formato con más tasa de error frente al de menos, entre los de al menos
/// [`MIN_FORMAT_RACES`] carreras. Hacen falta dos formatos: con filtro de formato no sale.
fn format_insight(history: &History) -> Option<Insight> {
    let groups: Vec<(RaceFormat, usize, f64)> = history
        .by_format
        .iter()
        .filter(|g| g.stats.races >= MIN_FORMAT_RACES)
        .filter_map(|g| Some((g.format?, g.stats.races, g.stats.error_rate?)))
        .collect();
    let by_rate =
        |a: &&(RaceFormat, usize, f64), b: &&(RaceFormat, usize, f64)| a.2.total_cmp(&b.2);
    // A igual tasa, `max_by` da el último y `min_by` el primero: siempre dos formatos distintos.
    let worst = groups.iter().max_by(by_rate)?;
    let best = groups.iter().min_by(by_rate)?;
    if worst.0 == best.0 || !clearly_higher(worst.2, best.2) {
        return None;
    }
    let text = format!(
        "Fallas más en {} que en {}: el {} de tus tramos son error, frente al {}.",
        format_name(worst.0),
        format_name(best.0),
        pct(worst.2),
        pct(best.2)
    );
    let races = worst.1.min(best.1);
    let caveat = (races < SUFFICIENT_RACES).then_some(FEW_RACES);
    Some(Insight::new(
        InsightRule::Format,
        InsightTarget::Formats,
        text,
        caveat,
    ))
}

fn format_name(f: RaceFormat) -> &'static str {
    match f {
        RaceFormat::Sprint => "sprint",
        RaceFormat::Middle => "media",
        RaceFormat::Long => "larga",
    }
}

/// P11: el cubo de más de 7 días sin competir con menos IR de entrada en mapa, frente al IR
/// medio del total (la misma referencia que la pantalla).
fn days_off_insight(days: &DaysOff, history: &History) -> Option<Insight> {
    let mean = history.total.mean_performance?;
    let (bucket, ir) = days
        .buckets
        .iter()
        // Solo tras descansar: más de una semana.
        .filter(|b| b.from_days > 7)
        .filter(|b| b.races >= MIN_DAYS_OFF_RACES && b.first_legs >= MIN_LEGS)
        .filter_map(|b| Some((b, b.first_legs_performance?)))
        .fold(None, |w: Option<(_, f64)>, (b, ir)| match w {
            Some((_, wir)) if wir <= ir => w,
            _ => Some((b, ir)),
        })?;
    if !at_least(mean - ir, PERFORMANCE_GAP) {
        return None;
    }
    let days_label = match bucket.to_days {
        Some(to) => format!("entre {} y {to} días", bucket.from_days),
        None => format!("más de {} días", bucket.from_days - 1),
    };
    let text = format!(
        "Tras {days_label} sin competir entras peor en mapa: rindes al {} en los tres primeros \
         tramos, frente a tu {} de media.",
        pct(ir),
        pct(mean)
    );
    let caveat = (bucket.races < SUFFICIENT_RACES).then_some(FEW_RACES);
    Some(Insight::new(
        InsightRule::DaysOff,
        InsightTarget::DaysOff,
        text,
        caveat,
    ))
}

// --- Carrera ----------------------------------------------------------------------------------

/// P1: carrera sin ningún error.
fn clean_race_insight(report: &RunnerReport, caveat: Option<&str>) -> Option<Insight> {
    (report.lost_time.error_count == 0).then(|| {
        Insight::new(
            InsightRule::CleanRace,
            InsightTarget::Legs,
            "Carrera limpia: no fallaste en ningún tramo.".to_string(),
            caveat,
        )
    })
}

/// P1: uno o dos tramos con error se llevan al menos [`CONCENTRATED_SHARE`] del tiempo
/// perdido, habiendo más errores que esos (si no, sería obvio).
fn concentrated_loss_insight(report: &RunnerReport, caveat: Option<&str>) -> Option<Insight> {
    let lost = &report.lost_time;
    let lost_s = lost.lost_time_s.filter(|s| *s > 0.0)?;
    let mut errors: Vec<(usize, f64)> = lost
        .legs
        .iter()
        .filter(|l| l.is_error)
        .filter_map(|l| Some((l.index, l.loss_s?)))
        .collect();
    // De más a menos pérdida; a igual pérdida, por número de tramo.
    errors.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let (k, sum) = (1..=2)
        .filter(|&k| errors.len() > k)
        .map(|k| (k, errors.iter().take(k).map(|e| e.1).sum::<f64>()))
        .find(|&(_, sum)| at_least(sum / lost_s, CONCENTRATED_SHARE))?;
    let mut legs: Vec<usize> = errors.iter().take(k).map(|e| e.0).collect();
    legs.sort_unstable();
    let share = pct(sum / lost_s);
    let amounts = format!("{} de {}", clock(sum), clock(lost_s));
    let text = match legs.as_slice() {
        [one] => {
            format!("Un solo tramo, el {one}, se llevó el {share} del tiempo perdido ({amounts}).")
        }
        [a, b] => format!(
            "Dos tramos, el {a} y el {b}, se llevaron el {share} del tiempo perdido ({amounts})."
        ),
        _ => return None,
    };
    Some(Insight::new(
        InsightRule::ConcentratedLoss,
        InsightTarget::Legs,
        text,
        caveat,
    ))
}

/// P5: la racha perdiendo que más costó, si tiene al menos [`STREAK_LEGS`] tramos y
/// [`STREAK_LOSS_S`] segundos.
fn losing_streak_insight(report: &RunnerReport, caveat: Option<&str>) -> Option<Insight> {
    let streak = report
        .lost_time
        .losing_streaks
        .iter()
        .filter(|s| s.last_leg + 1 - s.first_leg >= STREAK_LEGS && s.loss_s >= STREAK_LOSS_S)
        .max_by(|a, b| a.loss_s.total_cmp(&b.loss_s))?;
    let text = format!(
        "Del tramo {} al {} encadenaste {} tramos perdiendo tiempo: {} en total.",
        streak.first_leg,
        streak.last_leg,
        streak.last_leg + 1 - streak.first_leg,
        clock(streak.loss_s)
    );
    Some(Insight::new(
        InsightRule::LosingStreak,
        InsightTarget::GainLoss,
        text,
        caveat,
    ))
}

/// La mayoría de los errores (al menos [`THIRD_SHARE`] y [`THIRD_MIN_ERRORS`] errores) en un
/// mismo tercio de la carrera, por número de tramos (`history::race_third`).
fn errors_by_third_insight(report: &RunnerReport, caveat: Option<&str>) -> Option<Insight> {
    let legs = &report.lost_time.legs;
    let mut thirds = [0usize; 3];
    for leg in legs.iter().filter(|l| l.is_error) {
        if let Some(t) = thirds.get_mut(race_third(leg.index, legs.len())) {
            *t += 1;
        }
    }
    let errors: usize = thirds.iter().sum();
    if errors < THIRD_MIN_ERRORS {
        return None;
    }
    // Con al menos dos tercios en uno, no puede haber empate.
    let (third, count) = thirds.iter().enumerate().max_by_key(|(_, c)| **c)?;
    let (num, den) = THIRD_SHARE;
    if count * den < num * errors {
        return None;
    }
    let when = match third {
        0 => "al principio de la carrera (primer tercio)",
        1 => "en la mitad de la carrera (segundo tercio)",
        _ => "al final de la carrera (último tercio)",
    };
    let text = format!("La mayoría de tus errores ({count} de {errors}) llegaron {when}.");
    Some(Insight::new(
        InsightRule::ErrorsByThird,
        InsightTarget::Legs,
        text,
        caveat,
    ))
}

// --- Formato de los números -------------------------------------------------------------------

/// Una parte (0–1) en % sin decimales: `0,3` → «30 %».
fn pct(rate: f64) -> String {
    format!("{:.0} %", (rate * 100.0).round() + 0.0)
}

/// Duración redondeada al segundo: `m:ss`, o `h:mm:ss` desde una hora.
fn clock(seconds: f64) -> String {
    let total = seconds.abs().round() as u64;
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::after_error::Rate;
    use crate::common_errors::{ErrorTypes, TypeCount};
    use crate::days_off::DaysOffStats;
    use crate::gain_loss::{leg_gains, losing_streaks};
    use crate::history::{FormatHistory, HistoryFilter, HistoryStats};
    use crate::leg_length::BUCKET_BOUNDS_S;
    use crate::loss_breakdown::BreakdownTotals;
    use crate::runner_report::{CourseSummary, LegReport, RunnerLostTime};
    use crate::slope::SlopeConfig;

    // --- Datos sintéticos del histórico -------------------------------------------------------

    fn ratio(a: usize, b: usize) -> Option<f64> {
        (b > 0).then(|| a as f64 / b as f64)
    }

    fn stats(races: usize, legs: usize, errors: usize, performance: f64) -> HistoryStats {
        HistoryStats {
            races,
            legs,
            errors,
            mean_performance: Some(performance),
            error_rate: ratio(errors, legs),
            ..HistoryStats::default()
        }
    }

    /// Histórico con `races` carreras, una tasa de error total de `errors / legs` y un IR medio
    /// del 92 %.
    fn history(races: usize, legs: usize, errors: usize) -> History {
        history_with(stats(races, legs, errors, 0.92), Vec::new())
    }

    fn history_with(total: HistoryStats, by_format: Vec<FormatHistory>) -> History {
        History {
            filter: HistoryFilter::default(),
            by_format,
            total,
            races_without_data: 0,
        }
    }

    /// Cubo `i` de duración (P7) con `legs` tramos y `errors` errores.
    fn bucket(i: usize, legs: usize, errors: usize) -> LegLengthStats {
        LegLengthStats {
            from_s: BUCKET_BOUNDS_S[i],
            to_s: BUCKET_BOUNDS_S.get(i + 1).copied(),
            legs,
            errors,
            error_rate: ratio(errors, legs),
            mean_loss_s: None,
            mean_loss_pct: None,
        }
    }

    fn type_count(key: &str, errors: usize) -> TypeCount {
        TypeCount {
            error_type: key.into(),
            errors,
            loss_s: 0.0,
            subtypes: Vec::new(),
        }
    }

    /// Errores con `untyped` sin tipo y el resto repartido en `by_type` (de más a menos).
    fn common(untyped: usize, by_type: Vec<TypeCount>) -> CommonErrors {
        let typed: usize = by_type.iter().map(|t| t.errors).sum();
        CommonErrors {
            total: ErrorTypes {
                legs: 100,
                errors: typed + untyped,
                loss_s: 0.0,
                untyped,
                untyped_loss_s: 0.0,
                unreviewed: 0,
                by_type,
                physical_legs: 0,
                physical_loss_s: 0.0,
            },
            by_leg_length: Vec::new(),
            by_format: Vec::new(),
        }
    }

    fn rate(legs: usize, errors: usize) -> Rate {
        Rate {
            legs,
            errors,
            error_rate: ratio(errors, legs),
        }
    }

    fn after(after_error: Rate, after_clean: Rate) -> AfterError {
        AfterError {
            after_error,
            after_clean,
            accelerated: Rate::default(),
            not_accelerated: Rate::default(),
            after_error_without_speed: 0,
            streaks: Vec::new(),
        }
    }

    /// Reparto (P2) de `legs` errores: sus tres partes (s); la pérdida es su suma.
    fn breakdown(
        races_with_track: usize,
        legs: usize,
        detour_s: f64,
        stopped_s: f64,
        pace_s: f64,
    ) -> BreakdownHistory {
        BreakdownHistory {
            errors: BreakdownTotals {
                legs,
                loss_s: detour_s + stopped_s + pace_s,
                detour_s,
                stopped_s,
                pace_s,
            },
            races_with_track,
            races_without_track: 0,
            errors_without_breakdown: 0,
        }
    }

    fn class(class: SlopeClass, legs: usize, ir: f64) -> SlopeStats {
        SlopeStats {
            class,
            legs,
            errors: 0,
            error_rate: Some(0.0),
            mean_performance: Some(ir),
            reference_s: 60.0 * legs as f64,
        }
    }

    /// Desnivel (P13): (tramos, IR) en subida, llano y bajada.
    fn slope(
        races: usize,
        up: (usize, f64),
        flat: (usize, f64),
        down: (usize, f64),
    ) -> SlopeHistory {
        SlopeHistory {
            config: SlopeConfig::default(),
            by_class: vec![
                class(SlopeClass::Uphill, up.0, up.1),
                class(SlopeClass::Flat, flat.0, flat.1),
                class(SlopeClass::Downhill, down.0, down.1),
            ],
            races_with_track: races,
            races_without_track: 0,
            legs_without_track: 0,
            unclassified_legs: 0,
        }
    }

    fn format(format: RaceFormat, races: usize, legs: usize, errors: usize) -> FormatHistory {
        FormatHistory {
            format: Some(format),
            stats: stats(races, legs, errors, 0.9),
        }
    }

    /// Cubo de días sin competir (P11) con su IR de entrada en mapa.
    fn days(from: i64, to: Option<i64>, races: usize, first_legs: usize, ir: f64) -> DaysOffStats {
        DaysOffStats {
            from_days: from,
            to_days: to,
            races,
            first_legs,
            first_legs_performance: Some(ir),
            first_third_legs: 0,
            first_third_errors: 0,
            first_third_error_rate: None,
        }
    }

    fn days_off(buckets: Vec<DaysOffStats>) -> DaysOff {
        DaysOff {
            buckets,
            without_previous: 0,
        }
    }

    fn caveat(i: &Insight) -> Option<&str> {
        i.caveat.as_deref()
    }

    // --- P7: duración del tramo ---------------------------------------------------------------

    /// Total: 15 errores en 100 tramos (15 %). El cubo de 4–8 min, 8 de 20 (40 %): 25 puntos más
    /// y 2,7 veces la media.
    #[test]
    fn leg_length_fires_on_the_worst_bucket() {
        let buckets = [bucket(0, 30, 3), bucket(2, 50, 4), bucket(4, 20, 8)];
        let i = leg_length_insight(&buckets, &history(10, 100, 15)).unwrap();
        assert_eq!(i.rule, InsightRule::LegLength);
        assert_eq!(i.target, InsightTarget::LegLength);
        assert_eq!(
            i.text,
            "Fallas más en los tramos largos (de 4 a 8 min): el 40 % son error, frente al 15 % \
             de media."
        );
        assert!(!i.few_data);
        assert_eq!(caveat(&i), None);
    }

    /// 20 % frente a 15 %: solo 5 puntos. Y el cubo de 75 % tiene 4 tramos, menos del mínimo.
    #[test]
    fn leg_length_does_not_fire_without_a_clear_gap() {
        let buckets = [bucket(1, 4, 3), bucket(2, 76, 9), bucket(4, 20, 4)];
        assert_eq!(leg_length_insight(&buckets, &history(10, 100, 15)), None);
    }

    /// 4 de 6 (67 %) en 30 s–1 min: sale, pero con pocos tramos; con 3 carreras, con pocas
    /// carreras.
    #[test]
    fn leg_length_with_few_data_carries_a_caveat() {
        let buckets = [bucket(1, 6, 4), bucket(3, 94, 11)];
        let i = leg_length_insight(&buckets, &history(10, 100, 15)).unwrap();
        assert_eq!(
            i.text,
            "Fallas más en los tramos cortos (de 30 s a 1 min): el 67 % son error, frente al 15 % \
             de media."
        );
        assert!(i.few_data);
        assert_eq!(caveat(&i), Some(FEW_LEGS));

        let buckets = [bucket(5, 20, 8)];
        let i = leg_length_insight(&buckets, &history(3, 100, 15)).unwrap();
        assert!(i.text.contains("largos (de 8 min o más)"));
        assert_eq!(caveat(&i), Some(FEW_RACES));
    }

    // --- P9: errores más comunes --------------------------------------------------------------

    /// 12 errores, 2 sin tipo: de los 10 con tipo, 5 de navegación (50 %).
    #[test]
    fn common_error_fires_when_a_type_dominates() {
        let taxonomy = Taxonomy::builtin().unwrap();
        let errors = common(
            2,
            vec![
                type_count("navigation", 5),
                type_count("attack", 3),
                type_count("route_choice", 2),
            ],
        );
        let i = common_error_insight(&errors, 10, Some(&taxonomy)).unwrap();
        assert_eq!(i.rule, InsightRule::CommonError);
        assert_eq!(i.target, InsightTarget::CommonErrors);
        assert_eq!(
            i.text,
            "Tu error más común: navegación (5 de tus 10 errores con tipo)."
        );
        assert!(!i.few_data);
    }

    /// 3 de 10 (30 %) no llega al 40 %; y 2 errores con tipo no llegan al mínimo.
    #[test]
    fn common_error_does_not_fire_when_errors_are_spread() {
        let errors = common(
            0,
            vec![
                type_count("navigation", 3),
                type_count("attack", 3),
                type_count("route_choice", 2),
                type_count("control_exit", 2),
            ],
        );
        assert_eq!(common_error_insight(&errors, 10, None), None);
        let errors = common(5, vec![type_count("navigation", 2)]);
        assert_eq!(common_error_insight(&errors, 10, None), None);
    }

    /// 3 de 4 errores con tipo: sale con pocos errores. Sin taxonomía, la clave.
    #[test]
    fn common_error_with_few_data_carries_a_caveat() {
        let errors = common(
            1,
            vec![type_count("navigation", 3), type_count("attack", 1)],
        );
        let i = common_error_insight(&errors, 10, None).unwrap();
        assert_eq!(
            i.text,
            "Tu error más común: navigation (3 de tus 4 errores con tipo)."
        );
        assert_eq!(caveat(&i), Some(FEW_ERRORS));
        let i = common_error_insight(&errors, 4, None).unwrap();
        assert_eq!(caveat(&i), Some(FEW_RACES));
    }

    // --- P8: después de fallar ----------------------------------------------------------------

    /// Tras un error, 8 de 20 (40 %); tras un limpio, 12 de 80 (15 %).
    #[test]
    fn after_error_fires_when_errors_chain() {
        let i = after_error_insight(&after(rate(20, 8), rate(80, 12)), 10).unwrap();
        assert_eq!(i.rule, InsightRule::AfterError);
        assert_eq!(i.target, InsightTarget::AfterError);
        assert_eq!(
            i.text,
            "Un error suele traer otro: tras fallar, fallas el 40 % de los tramos; tras un tramo \
             limpio, el 15 %."
        );
        assert!(!i.few_data);
    }

    /// 20 % frente a 15 %; y 3 de 4 tras un error, por debajo del mínimo de tramos.
    #[test]
    fn after_error_does_not_fire_without_chaining() {
        let few_more = after(rate(20, 4), rate(80, 12));
        assert_eq!(after_error_insight(&few_more, 10), None);
        let too_few = after(rate(4, 3), rate(80, 12));
        assert_eq!(after_error_insight(&too_few, 10), None);
    }

    /// 3 de 6 (50 %) frente a 5 de 50 (10 %): con pocos errores.
    #[test]
    fn after_error_with_few_data_carries_a_caveat() {
        let data = after(rate(6, 3), rate(50, 5));
        let i = after_error_insight(&data, 10).unwrap();
        assert!(
            i.text
                .contains("el 50 % de los tramos; tras un tramo limpio, el 10 %")
        );
        assert_eq!(caveat(&i), Some(FEW_ERRORS));
        let i = after_error_insight(&data, 2).unwrap();
        assert_eq!(caveat(&i), Some(FEW_RACES));
    }

    // --- P2: ¿lento o desorientado? -----------------------------------------------------------

    /// 300 s en 12 errores: desvío 180 (60 %), paradas 60, ritmo 60. Y las otras dos partes.
    #[test]
    fn loss_breakdown_fires_on_the_main_part() {
        let i = loss_breakdown_insight(&breakdown(6, 12, 180.0, 60.0, 60.0)).unwrap();
        assert_eq!(i.rule, InsightRule::LossBreakdown);
        assert_eq!(i.target, InsightTarget::LossBreakdown);
        assert_eq!(
            i.text,
            "Cuando fallas, sobre todo te desvías: el 60 % del tiempo de tus errores es por \
             correr de más."
        );
        assert!(!i.few_data);

        // Paradas: 200 de 300 s.
        let i = loss_breakdown_insight(&breakdown(6, 12, 50.0, 200.0, 50.0)).unwrap();
        assert!(
            i.text
                .starts_with("Cuando fallas, sobre todo te paras: el 67 %")
        );
        // Ritmo: 250 de 300 s. Un desvío negativo (más directo de lo habitual) resta.
        let i = loss_breakdown_insight(&breakdown(6, 12, -20.0, 70.0, 250.0)).unwrap();
        assert!(
            i.text
                .starts_with("Cuando fallas, sobre todo vas más lento: el 83 %")
        );
    }

    /// 40 % de desvío no llega a la mitad; y 2 errores no llegan al mínimo.
    #[test]
    fn loss_breakdown_does_not_fire_when_mixed() {
        assert_eq!(
            loss_breakdown_insight(&breakdown(6, 12, 120.0, 90.0, 90.0)),
            None
        );
        assert_eq!(
            loss_breakdown_insight(&breakdown(6, 2, 300.0, 0.0, 0.0)),
            None
        );
    }

    /// 4 errores: con pocos errores; con 2 carreras con track, con pocas carreras.
    #[test]
    fn loss_breakdown_with_few_data_carries_a_caveat() {
        let i = loss_breakdown_insight(&breakdown(6, 4, 100.0, 0.0, 0.0)).unwrap();
        assert!(i.text.contains("el 100 %"));
        assert_eq!(caveat(&i), Some(FEW_ERRORS));
        let i = loss_breakdown_insight(&breakdown(2, 12, 100.0, 0.0, 0.0)).unwrap();
        assert_eq!(caveat(&i), Some(FEW_RACES));
    }

    // --- P13: desnivel ------------------------------------------------------------------------

    /// Subida 72 %, llano 90 %, bajada 85 %: la subida, 18 puntos por debajo.
    #[test]
    fn slope_fires_on_uphill_or_downhill() {
        let i = slope_insight(&slope(6, (20, 0.72), (30, 0.90), (15, 0.85))).unwrap();
        assert_eq!(i.rule, InsightRule::Slope);
        assert_eq!(i.target, InsightTarget::Slope);
        assert_eq!(
            i.text,
            "Las subidas te frenan: rindes al 72 % en subida y al 90 % en llano."
        );
        assert!(!i.few_data);

        let i = slope_insight(&slope(6, (20, 0.88), (30, 0.90), (15, 0.75))).unwrap();
        assert_eq!(
            i.text,
            "Las bajadas te frenan: rindes al 75 % en bajada y al 90 % en llano."
        );
    }

    /// 84 % frente a 90 %: 6 puntos. La bajada de 50 % tiene 4 tramos, menos del mínimo.
    #[test]
    fn slope_does_not_fire_without_a_clear_gap() {
        let s = slope(6, (20, 0.84), (30, 0.90), (4, 0.50));
        assert_eq!(slope_insight(&s), None);
    }

    /// 6 tramos de subida: con pocos tramos; 3 carreras con track: con pocas carreras.
    #[test]
    fn slope_with_few_data_carries_a_caveat() {
        let i = slope_insight(&slope(6, (6, 0.70), (30, 0.90), (15, 0.88))).unwrap();
        assert_eq!(caveat(&i), Some(FEW_LEGS));
        let i = slope_insight(&slope(3, (20, 0.70), (30, 0.90), (15, 0.88))).unwrap();
        assert_eq!(caveat(&i), Some(FEW_RACES));
    }

    // --- P6: formato --------------------------------------------------------------------------

    /// Sprint 10 de 100 (10 %), media 15 de 60 (25 %); la larga, con una carrera, no cuenta.
    #[test]
    fn format_fires_on_the_worst_format() {
        let h = history_with(
            stats(12, 170, 33, 0.9),
            vec![
                format(RaceFormat::Sprint, 6, 100, 10),
                format(RaceFormat::Middle, 5, 60, 15),
                format(RaceFormat::Long, 1, 10, 8),
            ],
        );
        let i = format_insight(&h).unwrap();
        assert_eq!(i.rule, InsightRule::Format);
        assert_eq!(i.target, InsightTarget::Formats);
        assert_eq!(
            i.text,
            "Fallas más en media que en sprint: el 25 % de tus tramos son error, frente al 10 %."
        );
        assert!(!i.few_data);
    }

    /// 18 % frente a 10 %: 8 puntos. Con filtro de formato (un solo grupo) no hay con qué
    /// comparar.
    #[test]
    fn format_does_not_fire_without_two_clearly_different_formats() {
        let h = history_with(
            stats(11, 150, 19, 0.9),
            vec![
                format(RaceFormat::Sprint, 6, 100, 10),
                format(RaceFormat::Middle, 5, 50, 9),
            ],
        );
        assert_eq!(format_insight(&h), None);
        let h = history_with(
            stats(5, 60, 15, 0.9),
            vec![format(RaceFormat::Middle, 5, 60, 15)],
        );
        assert_eq!(format_insight(&h), None);
    }

    /// La larga, con 2 carreras: con pocas carreras.
    #[test]
    fn format_with_few_data_carries_a_caveat() {
        let h = history_with(
            stats(8, 130, 18, 0.9),
            vec![
                format(RaceFormat::Sprint, 6, 100, 10),
                format(RaceFormat::Long, 2, 30, 8),
            ],
        );
        let i = format_insight(&h).unwrap();
        assert!(
            i.text
                .starts_with("Fallas más en larga que en sprint: el 27 %")
        );
        assert_eq!(caveat(&i), Some(FEW_RACES));
    }

    // --- P11: días sin competir ---------------------------------------------------------------

    /// IR medio 92 %. Tras más de 30 días, 80 % en los tres primeros tramos. El cubo de hasta 7
    /// días (70 %) no cuenta: no es descansar.
    #[test]
    fn days_off_fires_after_a_long_break() {
        let d = days_off(vec![
            days(1, Some(7), 10, 30, 0.70),
            days(8, Some(14), 5, 15, 0.90),
            days(31, None, 5, 15, 0.80),
        ]);
        let i = days_off_insight(&d, &history(20, 300, 30)).unwrap();
        assert_eq!(i.rule, InsightRule::DaysOff);
        assert_eq!(i.target, InsightTarget::DaysOff);
        assert_eq!(
            i.text,
            "Tras más de 30 días sin competir entras peor en mapa: rindes al 80 % en los tres \
             primeros tramos, frente a tu 92 % de media."
        );
        assert!(!i.few_data);
    }

    /// 85 % frente a 92 %: 7 puntos. Y 50 % con una sola carrera, por debajo del mínimo.
    #[test]
    fn days_off_does_not_fire_without_a_clear_drop() {
        let d = days_off(vec![
            days(1, Some(7), 10, 30, 0.50),
            days(15, Some(30), 1, 3, 0.50),
            days(31, None, 5, 15, 0.85),
        ]);
        assert_eq!(days_off_insight(&d, &history(20, 300, 30)), None);
    }

    /// 2 carreras de 15 a 30 días al 75 %: con pocas carreras.
    #[test]
    fn days_off_with_few_data_carries_a_caveat() {
        let d = days_off(vec![days(15, Some(30), 2, 6, 0.75)]);
        let i = days_off_insight(&d, &history(20, 300, 30)).unwrap();
        assert!(
            i.text
                .starts_with("Tras entre 15 y 30 días sin competir entras peor en mapa")
        );
        assert_eq!(caveat(&i), Some(FEW_RACES));
    }

    // --- Selección en el histórico ------------------------------------------------------------

    /// Análisis que disparan las siete reglas, todas con datos suficientes.
    struct AllRules {
        history: History,
        buckets: Vec<LegLengthStats>,
        slope: SlopeHistory,
        common: CommonErrors,
        after: AfterError,
        days: DaysOff,
        breakdown: BreakdownHistory,
    }

    impl AllRules {
        fn new() -> Self {
            AllRules {
                history: history_with(
                    stats(11, 100, 15, 0.92),
                    vec![
                        format(RaceFormat::Sprint, 6, 60, 6),
                        format(RaceFormat::Middle, 5, 40, 10),
                    ],
                ),
                buckets: vec![bucket(0, 80, 7), bucket(4, 20, 8)],
                slope: slope(6, (20, 0.72), (30, 0.90), (15, 0.85)),
                common: common(
                    2,
                    vec![type_count("navigation", 5), type_count("attack", 5)],
                ),
                after: after(rate(20, 8), rate(80, 12)),
                days: days_off(vec![days(31, None, 5, 15, 0.80)]),
                breakdown: breakdown(6, 12, 180.0, 60.0, 60.0),
            }
        }

        fn insights(&self) -> Vec<Insight> {
            history_insights(&HistoryAnalyses {
                history: &self.history,
                by_leg_length: &self.buckets,
                by_slope: &self.slope,
                common_errors: &self.common,
                after_error: &self.after,
                days_off: &self.days,
                loss_breakdown: &self.breakdown,
                taxonomy: None,
            })
        }
    }

    fn rules(insights: &[Insight]) -> Vec<InsightRule> {
        insights.iter().map(|i| i.rule).collect()
    }

    #[test]
    fn history_keeps_the_three_most_important() {
        let all = AllRules::new();
        assert_eq!(
            rules(&all.insights()),
            [
                InsightRule::LegLength,
                InsightRule::CommonError,
                InsightRule::AfterError
            ]
        );
    }

    /// Una frase con pocos datos va detrás de todas las que tienen datos suficientes, aunque su
    /// regla pese más.
    #[test]
    fn history_puts_few_data_last() {
        let mut all = AllRules::new();
        all.buckets = vec![bucket(0, 94, 11), bucket(4, 6, 4)];
        all.common = common(0, Vec::new());
        all.after = after(rate(20, 3), rate(80, 12));
        all.breakdown = breakdown(6, 12, 100.0, 100.0, 100.0);
        assert_eq!(
            rules(&all.insights()),
            [
                InsightRule::Slope,
                InsightRule::Format,
                InsightRule::DaysOff
            ]
        );
        all.slope = slope(6, (20, 0.88), (30, 0.90), (15, 0.88));
        all.history.by_format.clear();
        let insights = all.insights();
        assert_eq!(
            rules(&insights),
            [InsightRule::DaysOff, InsightRule::LegLength]
        );
        assert!(!insights[0].few_data);
        assert!(insights[1].few_data);
    }

    #[test]
    fn history_without_patterns_has_no_insights() {
        let mut all = AllRules::new();
        all.history = history(0, 0, 0);
        all.history.total.mean_performance = None;
        all.buckets = Vec::new();
        all.slope = slope(0, (0, 0.0), (0, 0.0), (0, 0.0));
        all.common = common(0, Vec::new());
        all.after = after(Rate::default(), Rate::default());
        all.days = days_off(Vec::new());
        all.breakdown = breakdown(0, 0, 0.0, 0.0, 0.0);
        assert!(all.insights().is_empty());
    }

    // --- Carrera ------------------------------------------------------------------------------

    /// Un tramo: `None` = sin pérdida; `Some((p, error))`.
    type Leg = Option<(f64, bool)>;

    /// Una carrera con un tramo por elemento. Las rachas salen de [`crate::gain_loss`], como en
    /// el informe de verdad.
    fn report(legs: &[Leg], weak_reference: bool) -> RunnerReport {
        let losses: Vec<Option<f64>> = legs.iter().map(|l| l.map(|(p, _)| p)).collect();
        let gains = leg_gains(&losses);
        let n = legs.len();
        let legs: Vec<LegReport> = legs
            .iter()
            .enumerate()
            .map(|(i, l)| LegReport {
                index: i + 1,
                from: 31,
                to: 32,
                split_s: l.map(|_| 60.0),
                elapsed_s: None,
                place: None,
                reference_s: Some(60.0),
                reference_count: 5,
                valid_splits: 20,
                performance_index: l.map(|_| 1.0),
                expected_s: l.map(|_| 60.0),
                loss_s: l.map(|(p, _)| p),
                loss_pct: None,
                is_error: l.is_some_and(|(_, e)| e),
                gain_s: None,
                cumulative_gain_s: None,
                ideal_elapsed_s: None,
                behind_ideal_s: None,
                is_last: i + 1 == n,
                short_reference: false,
                excluded_from_patterns: i + 1 == n,
            })
            .collect();
        let errors: Vec<&LegReport> = legs.iter().filter(|l| l.is_error).collect();
        RunnerReport {
            course: CourseSummary {
                controls: Vec::new(),
                classes: Vec::new(),
                valid_runners: if weak_reference { 3 } else { 20 },
                weak_reference,
            },
            lost_time: RunnerLostTime {
                total_s: None,
                usual_performance: Some(1.0),
                lost_time_s: Some(errors.iter().filter_map(|l| l.loss_s).sum()),
                error_count: errors.len(),
                time_without_errors_s: None,
                ideal_time_s: None,
                behind_ideal_s: None,
                losing_streaks: losing_streaks(&gains),
                consistency: None,
                legs,
            },
        }
    }

    const fn ok(p: f64) -> Leg {
        Some((p, false))
    }

    /// `n` tramos limpios que ganan 2 s cada uno.
    fn clean(n: usize) -> Vec<Leg> {
        vec![ok(-2.0); n]
    }

    /// `clean(n)` con los errores `(tramo, p)`.
    fn with_errors(n: usize, errors: &[(usize, f64)]) -> Vec<Leg> {
        let mut legs = clean(n);
        for &(index, p) in errors {
            legs[index - 1] = Some((p, true));
        }
        legs
    }

    #[test]
    fn clean_race_fires_without_errors() {
        let r = report(&clean(10), false);
        let i = clean_race_insight(&r, None).unwrap();
        assert_eq!(i.rule, InsightRule::CleanRace);
        assert_eq!(i.target, InsightTarget::Legs);
        assert_eq!(i.text, "Carrera limpia: no fallaste en ningún tramo.");
        let all = race_insights(&r);
        assert_eq!(rules(&all), [InsightRule::CleanRace]);
        assert!(!all[0].few_data);
    }

    #[test]
    fn clean_race_does_not_fire_with_an_error() {
        let r = report(&with_errors(10, &[(4, 40.0)]), false);
        assert_eq!(clean_race_insight(&r, None), None);
        // Un solo error: tampoco «pérdida concentrada», que sería obvio.
        assert!(race_insights(&r).is_empty());
    }

    /// 5 tramos con pérdida: con pocos tramos; con referencia débil, lo dice. Con 2 tramos o
    /// sin rendimiento habitual, ninguna frase.
    #[test]
    fn clean_race_with_few_data_carries_a_caveat() {
        let i = race_insights(&report(&clean(5), false));
        assert_eq!(rules(&i), [InsightRule::CleanRace]);
        assert_eq!(caveat(&i[0]), Some(FEW_LEGS));
        let i = race_insights(&report(&clean(10), true));
        assert_eq!(caveat(&i[0]), Some(WEAK_REFERENCE));
        assert!(i[0].few_data);
        assert!(race_insights(&report(&clean(2), false)).is_empty());
        let mut no_usual = report(&clean(10), false);
        no_usual.lost_time.usual_performance = None;
        assert!(race_insights(&no_usual).is_empty());
    }

    /// Errores de 60, 50, 30 y 20 s (2:40): el más caro, 37,5 %; los dos más caros, 110 s,
    /// 68,75 %.
    #[test]
    fn concentrated_loss_fires_on_one_or_two_legs() {
        let r = report(
            &with_errors(12, &[(7, 50.0), (3, 60.0), (9, 30.0), (10, 20.0)]),
            false,
        );
        let i = concentrated_loss_insight(&r, None).unwrap();
        assert_eq!(i.rule, InsightRule::ConcentratedLoss);
        assert_eq!(i.target, InsightTarget::Legs);
        assert_eq!(
            i.text,
            "Dos tramos, el 3 y el 7, se llevaron el 69 % del tiempo perdido (1:50 de 2:40)."
        );
        assert!(!i.few_data);

        // 90 de 120 s en un solo tramo.
        let r = report(&with_errors(12, &[(4, 90.0), (8, 30.0)]), false);
        assert_eq!(
            concentrated_loss_insight(&r, None).unwrap().text,
            "Un solo tramo, el 4, se llevó el 75 % del tiempo perdido (1:30 de 2:00)."
        );
    }

    /// Cinco errores de 40 s: dos son el 40 %. Con un solo error no se dice: sería obvio.
    #[test]
    fn concentrated_loss_does_not_fire_when_spread() {
        let five = [(2, 40.0), (4, 40.0), (6, 40.0), (8, 40.0), (10, 40.0)];
        let r = report(&with_errors(12, &five), false);
        assert_eq!(concentrated_loss_insight(&r, None), None);
        let r = report(&with_errors(12, &[(4, 90.0)]), false);
        assert_eq!(concentrated_loss_insight(&r, None), None);
    }

    #[test]
    fn concentrated_loss_with_few_data_carries_a_caveat() {
        let r = report(&with_errors(6, &[(2, 90.0), (5, 30.0)]), false);
        let i = race_insights(&r);
        assert_eq!(rules(&i), [InsightRule::ConcentratedLoss]);
        assert_eq!(caveat(&i[0]), Some(FEW_LEGS));
    }

    /// Tramos 2, 3 y 4 pierden 10, 12 y 15 s (37 s); los 7 y 8, 50 s, pero son solo 2.
    #[test]
    fn losing_streak_fires_on_a_long_streak() {
        let mut legs = clean(12);
        legs[1] = ok(10.0);
        legs[2] = ok(12.0);
        legs[3] = ok(15.0);
        legs[6] = ok(25.0);
        legs[7] = ok(25.0);
        let r = report(&legs, false);
        let i = losing_streak_insight(&r, None).unwrap();
        assert_eq!(i.rule, InsightRule::LosingStreak);
        assert_eq!(i.target, InsightTarget::GainLoss);
        assert_eq!(
            i.text,
            "Del tramo 2 al 4 encadenaste 3 tramos perdiendo tiempo: 0:37 en total."
        );
        assert!(!i.few_data);
    }

    /// Dos tramos seguidos (50 s) o tres que suman 20 s no son racha que contar.
    #[test]
    fn losing_streak_does_not_fire_on_short_or_cheap_streaks() {
        let mut legs = clean(12);
        legs[1] = ok(25.0);
        legs[2] = ok(25.0);
        legs[5] = ok(5.0);
        legs[6] = ok(5.0);
        legs[7] = ok(10.0);
        assert_eq!(losing_streak_insight(&report(&legs, false), None), None);
    }

    #[test]
    fn losing_streak_with_few_data_carries_a_caveat() {
        let legs = vec![ok(-5.0), ok(20.0), ok(20.0), ok(20.0), ok(-5.0)];
        let i = race_insights(&report(&legs, true));
        // Sin errores: también sale «carrera limpia», que va delante.
        assert_eq!(
            rules(&i),
            [InsightRule::CleanRace, InsightRule::LosingStreak]
        );
        assert!(i.iter().all(|i| caveat(i) == Some(WEAK_REFERENCE)));
    }

    /// 12 tramos (tercios 1–4, 5–8 y 9–12): errores en 2, 9, 10 y 12 → 3 de 4 al final.
    #[test]
    fn errors_by_third_fires_when_most_errors_are_together() {
        let r = report(
            &with_errors(12, &[(2, 30.0), (9, 30.0), (10, 30.0), (12, 30.0)]),
            false,
        );
        let i = errors_by_third_insight(&r, None).unwrap();
        assert_eq!(i.rule, InsightRule::ErrorsByThird);
        assert_eq!(i.target, InsightTarget::Legs);
        assert_eq!(
            i.text,
            "La mayoría de tus errores (3 de 4) llegaron al final de la carrera (último tercio)."
        );
        assert!(!i.few_data);
    }

    /// Uno en cada tercio; o solo dos errores.
    #[test]
    fn errors_by_third_does_not_fire_when_spread() {
        let r = report(&with_errors(12, &[(2, 30.0), (6, 30.0), (10, 30.0)]), false);
        assert_eq!(errors_by_third_insight(&r, None), None);
        let r = report(&with_errors(12, &[(1, 30.0), (2, 30.0)]), false);
        assert_eq!(errors_by_third_insight(&r, None), None);
    }

    /// 6 tramos (tercios 1–2, 3–4 y 5–6): errores en 1, 2 y 5 (justo dos tercios), con
    /// referencia débil.
    #[test]
    fn errors_by_third_with_few_data_carries_a_caveat() {
        let r = report(&with_errors(6, &[(1, 30.0), (2, 30.0), (5, 30.0)]), true);
        let i = race_insights(&r);
        let third = i
            .iter()
            .find(|i| i.rule == InsightRule::ErrorsByThird)
            .unwrap();
        assert_eq!(
            third.text,
            "La mayoría de tus errores (2 de 3) llegaron al principio de la carrera (primer \
             tercio)."
        );
        assert_eq!(caveat(third), Some(WEAK_REFERENCE));
    }

    /// Pérdida concentrada (90 de 125 s), racha del 8 al 11 y los tres errores al final: las
    /// tres, por prioridad.
    #[test]
    fn race_orders_by_priority() {
        let mut legs = with_errors(12, &[(9, 90.0), (10, 20.0), (11, 15.0)]);
        legs[7] = ok(5.0);
        let r = report(&legs, false);
        assert_eq!(
            rules(&race_insights(&r)),
            [
                InsightRule::ConcentratedLoss,
                InsightRule::LosingStreak,
                InsightRule::ErrorsByThird
            ]
        );
    }

    #[test]
    fn insights_serialize_with_snake_case_keys() {
        let i = Insight::new(
            InsightRule::LossBreakdown,
            InsightTarget::LossBreakdown,
            "Frase.".into(),
            Some(FEW_RACES),
        );
        assert_eq!(
            serde_json::to_value(&i).unwrap(),
            serde_json::json!({
                "rule": "loss_breakdown",
                "text": "Frase.",
                "few_data": true,
                "caveat": "con pocas carreras",
                "target": "loss_breakdown",
            })
        );
    }

    /// Los umbrales entran: 25 % frente a 15 % son justo 10 puntos (en binario, `0,0999…`), y
    /// 80 % frente a 90 %, justo 10 puntos de IR.
    #[test]
    fn thresholds_are_inclusive() {
        assert!(clearly_higher(0.25, 0.15));
        assert!(clearly_higher(0.30, 0.20));
        assert!(!clearly_higher(0.24, 0.15));
        assert!(slope_insight(&slope(6, (20, 0.80), (30, 0.90), (15, 0.90))).is_some());
        assert!(slope_insight(&slope(6, (20, 0.81), (30, 0.90), (15, 0.90))).is_none());
    }

    #[test]
    fn numbers_are_written_in_plain_spanish() {
        assert_eq!(pct(0.0), "0 %");
        assert_eq!(pct(-0.001), "0 %");
        assert_eq!(pct(0.6875), "69 %");
        assert_eq!(clock(37.4), "0:37");
        assert_eq!(clock(160.0), "2:40");
        assert_eq!(clock(3725.0), "1:02:05");
    }
}
