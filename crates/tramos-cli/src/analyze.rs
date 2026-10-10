//! `tramos analizar`: identifica al corredor en la carrera, calcula su tiempo perdido y, si hay
//! FIT, alinea el track con sus picadas. El formato de la salida está en `docs/cli.md`.

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use serde::Serialize;
use tramos_core::alignment::{
    Alignment, AlignmentOptions, AlignmentQuality, AlignmentWarning, Coverage, align,
};
use tramos_core::identify::{Candidate, Identification, RunnerIdentity, identify_runner};
use tramos_core::insights::{Insight, race_insights};
use tramos_core::loss_breakdown::{RaceBreakdown, race_breakdown};
use tramos_core::lost_time::LostTimeConfig;
use tramos_core::metrics::{MetricsOptions, leg_metrics};
use tramos_core::model::{Event, RaceStatus, Track};
use tramos_core::runner_report::{CourseSummary, RunnerLostTime, runner_report};
use tramos_core::segmentation::segment;

/// Cómo se busca al corredor: un `--corredor` numérico es la tarjeta SI; si no, el nombre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerQuery {
    SiCard(u32),
    Name(String),
}

impl RunnerQuery {
    pub fn parse(text: &str) -> Self {
        let text = text.trim();
        match text.parse::<u32>() {
            Ok(card) => Self::SiCard(card),
            Err(_) => Self::Name(text.to_string()),
        }
    }

    fn identity(&self) -> RunnerIdentity {
        match self {
            Self::SiCard(card) => RunnerIdentity {
                si_card: Some(*card),
                full_name: None,
            },
            Self::Name(name) => RunnerIdentity {
                si_card: None,
                full_name: Some(name.clone()),
            },
        }
    }
}

/// Salida de `tramos analizar`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Analysis {
    pub event: EventInfo,
    pub runner: RunnerInfo,
    pub course: CourseSummary,
    /// Umbrales y tiempo ideal con los que se ha calculado el tiempo perdido.
    pub config: LostTimeConfig,
    pub lost_time: RunnerLostTime,
    /// Resumen en frases de la carrera (`docs/frases.md`): como mucho tres. La de ¿lento o
    /// desorientado? solo sale con `--fit`.
    pub insights: Vec<Insight>,
    /// Solo si se ha pasado `--fit`.
    pub alignment: Option<AlignmentSummary>,
    /// Avisos de la identificación, en español.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EventInfo {
    pub name: Option<String>,
    pub date: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunnerInfo {
    /// Posición de la categoría en la carrera (como en `tramos_core`).
    pub class_index: usize,
    /// Posición del resultado en la categoría.
    pub result_index: usize,
    pub class_id: u32,
    pub class_name: String,
    pub given_name: String,
    pub family_name: String,
    pub club: Option<String>,
    pub bib: Option<u32>,
    pub si_card: Option<u32>,
    pub status: RaceStatus,
    pub place: Option<u16>,
}

/// La alineación del núcleo sin las picadas situadas en el track.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlignmentSummary {
    pub offset_s: f64,
    pub offset_estimated: bool,
    pub confidence: f64,
    pub quality: AlignmentQuality,
    pub coverage: Coverage,
    pub warnings: Vec<AlignmentWarning>,
}

/// Analiza al corredor `query` de `event` y, si hay `track`, lo alinea con sus picadas.
pub fn analyze(
    event: &Event,
    query: &RunnerQuery,
    config: &LostTimeConfig,
    track: Option<&Track>,
) -> Result<Analysis> {
    let mut warnings = Vec::new();
    let candidate = match identify_runner(event, &query.identity()) {
        Identification::Unique {
            candidate,
            name_mismatch,
        } => {
            if name_mismatch {
                warnings.push(format!(
                    "la tarjeta casa, pero el nombre no: {}",
                    describe(event, &candidate)
                ));
            }
            candidate
        }
        Identification::Ambiguous { candidates } => {
            let list: Vec<String> = candidates
                .iter()
                .map(|c| format!("  - {}", describe(event, c)))
                .collect();
            let hint = match query {
                RunnerQuery::SiCard(_) => "usa el nombre completo para elegir",
                RunnerQuery::Name(_) => "usa la tarjeta SI para elegir",
            };
            bail!(
                "{} casa con varios resultados ({hint}):\n{}",
                query_label(query),
                list.join("\n")
            );
        }
        Identification::NotFound => match query {
            RunnerQuery::SiCard(card) => {
                bail!("ningún resultado de la carrera lleva la tarjeta SI {card}")
            }
            RunnerQuery::Name(_) => bail!(
                "ningún resultado de la carrera casa con {}: hace falta el nombre y los \
                 apellidos completos, sin abreviar, o la tarjeta SI",
                query_label(query)
            ),
        },
    };

    let class_index = candidate.result.class_index;
    let result_index = candidate.result.result_index;
    let (Some(class), Some(result)) = (event.classes.get(class_index), candidate.result.get(event))
    else {
        bail!("error interno: la identificación apunta a un resultado que no existe");
    };
    let Some(report) = runner_report(event, candidate.result, config) else {
        bail!(
            "error interno: el corredor de {} no está en el análisis de su recorrido",
            class.name
        );
    };

    let alignment = track
        .map(|track| {
            align(track, result, &AlignmentOptions::default())
                .context("no se puede alinear el FIT con las picadas del corredor")
        })
        .transpose()?;
    let breakdown = match (track, &alignment) {
        (Some(track), Some(alignment)) => match breakdown_of(track, alignment, &report.lost_time) {
            Ok(breakdown) => Some(breakdown),
            Err(reason) => {
                warnings.push(format!(
                    "no se ha podido repartir la pérdida en desvío, paradas y ritmo: {reason}"
                ));
                None
            }
        },
        _ => None,
    };
    let insights = race_insights(&report, breakdown.as_ref());

    Ok(Analysis {
        event: EventInfo {
            name: event.name.clone(),
            date: event.date,
        },
        runner: RunnerInfo {
            class_index,
            result_index,
            class_id: class.id,
            class_name: class.name.clone(),
            given_name: result.runner.given_name.clone(),
            family_name: result.runner.family_name.clone(),
            club: result.runner.club.clone(),
            bib: result.runner.bib,
            si_card: result.runner.si_card,
            status: result.status,
            place: result.place,
        },
        course: report.course,
        config: *config,
        lost_time: report.lost_time,
        insights,
        alignment: alignment.map(align_summary),
        warnings,
    })
}

fn align_summary(alignment: Alignment) -> AlignmentSummary {
    AlignmentSummary {
        offset_s: alignment.offset_s,
        offset_estimated: alignment.offset_estimated,
        confidence: alignment.confidence,
        quality: alignment.quality,
        coverage: alignment.coverage,
        warnings: alignment.warnings,
    }
}

/// ¿Lento o desorientado? (P2): corta el track en tramos, mide cada uno y reparte la pérdida.
/// Si no se puede, el motivo (sin él, la carrera solo pierde esa frase).
fn breakdown_of(
    track: &Track,
    alignment: &Alignment,
    lost_time: &RunnerLostTime,
) -> Result<RaceBreakdown, String> {
    let segmentation = segment(track, alignment).map_err(|e| e.to_string())?;
    let metrics =
        leg_metrics(track, &segmentation, &MetricsOptions::default()).map_err(|e| e.to_string())?;
    Ok(race_breakdown(lost_time, &metrics))
}

fn query_label(query: &RunnerQuery) -> String {
    match query {
        RunnerQuery::SiCard(card) => format!("la tarjeta SI {card}"),
        RunnerQuery::Name(name) => format!("«{name}»"),
    }
}

/// Una línea por candidato: categoría, nombre, tarjeta y resultado.
fn describe(event: &Event, candidate: &Candidate) -> String {
    let class = event
        .classes
        .get(candidate.result.class_index)
        .map_or("?", |c| c.name.as_str());
    let Some(result) = candidate.result.get(event) else {
        return format!("{class}: ?");
    };
    let runner = &result.runner;
    let card = runner
        .si_card
        .map_or_else(|| "sin tarjeta".to_string(), |c| format!("tarjeta {c}"));
    format!(
        "{class}: {} {} ({card}, {})",
        runner.given_name,
        runner.family_name,
        status_label(result.status, result.place)
    )
}

/// Estado del corredor en español: «puesto 16», «no clasificado»…
pub fn status_label(status: RaceStatus, place: Option<u16>) -> String {
    match (status, place) {
        (RaceStatus::Ok, Some(place)) => format!("puesto {place}"),
        (RaceStatus::Ok, None) => "clasificado".to_string(),
        (RaceStatus::NotClassified, _) => "no clasificado".to_string(),
        (RaceStatus::DidNotStart, _) => "no presentado".to_string(),
        (RaceStatus::Unknown(code), _) => format!("no clasificado (código {code})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tramos_core::model::{Class, Course, RaceResult, Runner};

    #[test]
    fn numeric_query_is_si_card() {
        assert_eq!(RunnerQuery::parse(" 143 "), RunnerQuery::SiCard(143));
        assert_eq!(
            RunnerQuery::parse("Ana Pérez"),
            RunnerQuery::Name("Ana Pérez".to_string())
        );
        assert_eq!(
            RunnerQuery::parse("N143 Apellido143"),
            RunnerQuery::Name("N143 Apellido143".to_string())
        );
    }

    fn class(id: u32, name: &str, card: u32) -> Class {
        Class {
            id,
            name: name.to_string(),
            short_name: None,
            course: Course { controls: vec![31] },
            results: vec![RaceResult {
                runner: Runner {
                    given_name: "Ana".to_string(),
                    family_name: "Pérez".to_string(),
                    club: None,
                    bib: None,
                    si_card: Some(card),
                    sex: None,
                },
                status: RaceStatus::Ok,
                place: Some(1),
                punches: Vec::new(),
            }],
        }
    }

    /// Dos «Ana Pérez» en categorías distintas: el error las lista y pide la tarjeta.
    #[test]
    fn ambiguous_name_lists_candidates() {
        let event = Event {
            name: None,
            date: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
            classes: vec![class(1, "F-SEN", 10), class(2, "F-35", 20)],
        };
        let query = RunnerQuery::parse("ana perez");
        let error = analyze(&event, &query, &LostTimeConfig::default(), None).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("varios resultados"), "{message}");
        assert!(message.contains("tarjeta SI para elegir"), "{message}");
        assert!(
            message.contains("F-SEN: Ana Pérez (tarjeta 10, puesto 1)"),
            "{message}"
        );
        assert!(
            message.contains("F-35: Ana Pérez (tarjeta 20, puesto 1)"),
            "{message}"
        );

        let by_card = analyze(
            &event,
            &RunnerQuery::SiCard(20),
            &LostTimeConfig::default(),
            None,
        )
        .unwrap();
        assert_eq!(by_card.runner.class_name, "F-35");
        assert!(by_card.warnings.is_empty());
    }

    #[test]
    fn status_labels() {
        assert_eq!(status_label(RaceStatus::Ok, Some(3)), "puesto 3");
        assert_eq!(
            status_label(RaceStatus::NotClassified, None),
            "no clasificado"
        );
        assert_eq!(status_label(RaceStatus::DidNotStart, None), "no presentado");
        assert_eq!(
            status_label(RaceStatus::Unknown(7), None),
            "no clasificado (código 7)"
        );
    }
}
