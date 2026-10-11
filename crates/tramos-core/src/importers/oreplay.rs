//! Resultados de OReplay (<https://www.oreplay.es>): de su API pública al modelo de dominio.
//!
//! El núcleo no hace red: la app descarga las respuestas de la API (las URL salen de aquí,
//! [`event_url`], [`classes_url`] y [`results_url`]), las lee con [`parse_event`],
//! [`parse_classes`] y [`parse_results`] y junta lo que hace falta en una [`Download`]. La
//! descarga se guarda como JSON propio, con solo los campos que se usan, y [`read`] la convierte
//! en un [`Event`] igual que el lector del .spl. Formato y decisiones en `docs/formato-oreplay.md`.
//! Resumen:
//!
//! - Solo etapas individuales de tipo `Classic`: relevos, score y totales no tienen tramos
//!   comparables.
//! - Las horas vienen en UTC: no hay zona horaria que elegir ni cambio de hora que resolver. La
//!   zona del evento solo da el día de la carrera.
//! - El recorrido no viene aparte: es la secuencia de balizas (ordenada por `order_number`) más
//!   repetida entre los clasificados de la categoría.
//! - Se guarda lo mismo que del .spl; nada de lo que calcula OReplay (posiciones por tramo,
//!   tiempo perdido…).

use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::model::{
    Class, ControlCode, Course, Event, FINISH_CODE, Punch, RaceResult, RaceStatus, Runner,
    START_CODE, Sex,
};

/// Raíz de la API de OReplay.
pub const API_BASE: &str = "https://www.oreplay.es/api/v1";

/// Valor de [`Download::format`]: distingue una descarga de cualquier otro JSON.
pub const DOWNLOAD_FORMAT: &str = "tramos-oreplay";

/// Versión del formato de [`Download`] que escribe y lee este módulo.
pub const DOWNLOAD_VERSION: u32 = 1;

/// Tipo de etapa que se puede importar: carrera individual con recorrido fijo.
pub const CLASSIC_STAGE: &str = "Classic";

/// Zona del día de la carrera si el evento no trae una válida.
const DEFAULT_TIME_ZONE: Tz = chrono_tz::Europe::Madrid;

/// Errores al leer un enlace, una respuesta de la API o una descarga de OReplay. Los mensajes van
/// a la interfaz, en español.
#[derive(Debug, Error, PartialEq)]
pub enum OReplayError {
    #[error(
        "no es un enlace de una carrera de OReplay (se espera \
         https://www.oreplay.es/competitions/…): {0}"
    )]
    InvalidUrl(String),

    #[error("respuesta de OReplay no válida: {0}")]
    InvalidResponse(String),

    #[error("no es una descarga de OReplay de Tramos")]
    NotADownload,

    #[error("descarga de OReplay en un formato más nuevo ({0}): actualiza la app")]
    UnsupportedVersion(u32),

    #[error(
        "la etapa es de tipo «{0}»: solo se pueden importar carreras individuales (Classic), \
         no relevos, score ni totales"
    )]
    UnsupportedStage(String),

    #[error("la etapa no trae la fecha de la carrera")]
    MissingDate,

    #[error("en la categoría {class} hay una baliza con código no válido: {station:?}")]
    InvalidStation { class: String, station: String },

    #[error(
        "la carrera no tiene parciales: el organizador solo ha subido los tiempos de meta \
         (o aún no ha subido nada)"
    )]
    NoSplits,
}

/// Evento y etapa de un enlace de OReplay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub event_id: String,
    /// `None` si el enlace es el del evento, sin etapa.
    pub stage_id: Option<String>,
}

/// Lee un enlace de la web de OReplay: `https://www.oreplay.es/competitions/<evento>` o
/// `…/competitions/<evento>/<etapa>`, con o sin `www.`, y con lo que vaya detrás (más ruta,
/// `?…`, `#…`).
pub fn parse_url(url: &str) -> Result<Link, OReplayError> {
    let invalid = || OReplayError::InvalidUrl(url.trim().to_string());
    let rest = url.trim();
    let rest = rest
        .strip_prefix("https://")
        .or_else(|| rest.strip_prefix("http://"))
        .unwrap_or(rest);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    let path = rest.strip_prefix("oreplay.es/").ok_or_else(invalid)?;
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let mut parts = path.split('/').filter(|p| !p.is_empty());
    if parts.next() != Some("competitions") {
        return Err(invalid());
    }
    let event_id = parts.next().filter(|p| is_uuid(p)).ok_or_else(invalid)?;
    let stage_id = parts.next().filter(|p| is_uuid(p));
    Ok(Link {
        event_id: event_id.to_lowercase(),
        stage_id: stage_id.map(str::to_lowercase),
    })
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// URL del evento, con sus etapas.
pub fn event_url(event_id: &str) -> String {
    format!("{API_BASE}/events/{event_id}")
}

/// URL de las categorías de una etapa.
pub fn classes_url(event_id: &str, stage_id: &str) -> String {
    format!("{API_BASE}/events/{event_id}/stages/{stage_id}/classes")
}

/// URL de los resultados de una categoría, la misma llamada que hace la web de OReplay.
pub fn results_url(event_id: &str, stage_id: &str, class_id: &str) -> String {
    format!(
        "{API_BASE}/events/{event_id}/stages/{stage_id}/results?class_id={class_id}&forceSameDay=true"
    )
}

/// Respuesta de la API: lo útil va en `data`.
#[derive(Deserialize)]
struct Page<T> {
    data: T,
}

fn parse_page<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, OReplayError> {
    serde_json::from_slice::<Page<T>>(body)
        .map(|page| page.data)
        .map_err(|e| OReplayError::InvalidResponse(e.to_string()))
}

/// Lee la respuesta de [`event_url`].
pub fn parse_event(body: &[u8]) -> Result<ApiEvent, OReplayError> {
    parse_page(body)
}

/// Lee la respuesta de [`classes_url`].
pub fn parse_classes(body: &[u8]) -> Result<Vec<ApiClass>, OReplayError> {
    parse_page(body)
}

/// Lee la respuesta de [`results_url`].
pub fn parse_results(body: &[u8]) -> Result<Vec<ApiRunner>, OReplayError> {
    parse_page(body)
}

/// Evento de OReplay. Las etapas solo vienen en la respuesta de [`event_url`]; en la descarga
/// van vacías (la etapa elegida va aparte).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiEvent {
    pub id: String,
    /// Nombre del evento.
    #[serde(default)]
    pub description: Option<String>,
    /// Zona IANA del evento (`Europe/Madrid`).
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<ApiStage>,
}

impl ApiEvent {
    /// Las etapas que se pueden importar ([`CLASSIC_STAGE`]).
    pub fn classic_stages(&self) -> impl Iterator<Item = &ApiStage> {
        self.stages.iter().filter(|s| s.is_classic())
    }
}

/// Etapa de un evento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiStage {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Hora de salida de la etapa.
    #[serde(default)]
    pub start: Option<DateTime<Utc>>,
    #[serde(default)]
    pub stage_type: Option<ApiStageType>,
}

impl ApiStage {
    pub fn is_classic(&self) -> bool {
        self.type_name() == CLASSIC_STAGE
    }

    fn type_name(&self) -> &str {
        self.stage_type
            .as_ref()
            .and_then(|t| t.description.as_deref())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiStageType {
    #[serde(default)]
    pub description: Option<String>,
}

/// Categoría de una etapa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiClass {
    pub id: String,
    #[serde(default)]
    pub short_name: Option<String>,
    #[serde(default)]
    pub long_name: Option<String>,
}

/// Corredor con su resultado en la etapa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiRunner {
    /// Nombre y apellidos, en ese orden.
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub club: Option<ApiClub>,
    #[serde(default, deserialize_with = "text_or_number")]
    pub sicard: Option<String>,
    /// `M` o `F`.
    #[serde(default)]
    pub sex: Option<String>,
    #[serde(default, deserialize_with = "text_or_number")]
    pub bib_number: Option<String>,
    /// Fuera de concurso.
    #[serde(default)]
    pub is_nc: bool,
    /// Resultado en la etapa; sin él, el corredor no tomó la salida.
    #[serde(default)]
    pub stage: Option<ApiResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiClub {
    #[serde(default)]
    pub short_name: Option<String>,
}

/// Resultado de un corredor en la etapa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiResult {
    #[serde(default)]
    pub start_time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub finish_time: Option<DateTime<Utc>>,
    /// `0` OK, `1` no salió, `2` abandono, `3` falta baliza, `4` descalificado, `5` fuera de
    /// tiempo, `6` en carrera, `9` no competitivo, `F` llegado sin validar.
    #[serde(default, deserialize_with = "text_or_number")]
    pub status_code: Option<String>,
    #[serde(default)]
    pub position: Option<i64>,
    #[serde(default)]
    pub is_nc: bool,
    #[serde(default)]
    pub splits: Vec<ApiSplit>,
}

/// Picada de una baliza.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiSplit {
    /// Radio (paso intermedio retransmitido), no una baliza del recorrido.
    #[serde(default)]
    pub is_intermediate: bool,
    /// `None` si no se registró.
    #[serde(default)]
    pub reading_time: Option<DateTime<Utc>>,
    /// Posición en el recorrido; la API no las devuelve en orden.
    #[serde(default)]
    pub order_number: Option<i64>,
    #[serde(default)]
    pub control: Option<ApiControl>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiControl {
    /// Código de la baliza, como texto.
    #[serde(default, deserialize_with = "text_or_number")]
    pub station: Option<String>,
}

/// Acepta `"123"`, `123` o `null`: la API no es constante con los números.
fn text_or_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Ok(
        match Option::<serde_json::Value>::deserialize(deserializer)? {
            Some(serde_json::Value::String(text)) => Some(text),
            Some(serde_json::Value::Number(n)) => Some(n.to_string()),
            _ => None,
        },
    )
}

/// Categoría con sus corredores, dentro de una [`Download`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassResults {
    pub class: ApiClass,
    pub runners: Vec<ApiRunner>,
}

/// Una etapa descargada de OReplay, tal y como se guarda: lo que hace falta de la API y nada
/// más, para que dos descargas de los mismos resultados den el mismo fichero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Download {
    /// Siempre [`DOWNLOAD_FORMAT`].
    pub format: String,
    pub version: u32,
    pub event: ApiEvent,
    pub stage: ApiStage,
    pub classes: Vec<ClassResults>,
}

impl Download {
    /// Junta lo descargado. Las etapas del evento no se guardan: solo la elegida.
    pub fn new(event: ApiEvent, stage: ApiStage, classes: Vec<ClassResults>) -> Self {
        Self {
            format: DOWNLOAD_FORMAT.to_string(),
            version: DOWNLOAD_VERSION,
            event: ApiEvent {
                stages: Vec::new(),
                ..event
            },
            stage,
            classes,
        }
    }

    /// El JSON que se guarda.
    pub fn to_json(&self) -> Vec<u8> {
        // Serializar estructuras propias, sin mapas de claves no textuales, no falla.
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Lee una descarga guardada.
    pub fn from_json(data: &[u8]) -> Result<Self, OReplayError> {
        #[derive(Deserialize)]
        struct Header {
            format: Option<String>,
            version: Option<u32>,
        }
        let header: Header =
            serde_json::from_slice(data).map_err(|_| OReplayError::NotADownload)?;
        if header.format.as_deref() != Some(DOWNLOAD_FORMAT) {
            return Err(OReplayError::NotADownload);
        }
        match header.version {
            Some(DOWNLOAD_VERSION) => {}
            Some(v) => return Err(OReplayError::UnsupportedVersion(v)),
            None => return Err(OReplayError::NotADownload),
        }
        serde_json::from_slice(data).map_err(|e| OReplayError::InvalidResponse(e.to_string()))
    }

    /// Nombre de la carrera: el del evento y, si la etapa tiene uno distinto, el de la etapa.
    pub fn name(&self) -> Option<String> {
        let event = non_empty(self.event.description.as_deref());
        let stage = non_empty(self.stage.description.as_deref());
        match (event, stage) {
            (Some(e), Some(s)) if e != s => Some(format!("{e} · {s}")),
            (Some(e), _) => Some(e.to_string()),
            (None, s) => s.map(str::to_string),
        }
    }
}

/// Si `data` parece una descarga de OReplay (y no, por ejemplo, un .spl).
pub fn is_download(data: &[u8]) -> bool {
    data.iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|b| *b == b'{')
}

/// Lee una descarga guardada y la convierte al modelo de dominio.
pub fn read(data: &[u8]) -> Result<Event, OReplayError> {
    to_event(&Download::from_json(data)?)
}

/// Convierte una descarga al modelo de dominio (ver el módulo y `docs/formato-oreplay.md`).
pub fn to_event(download: &Download) -> Result<Event, OReplayError> {
    if !download.stage.is_classic() {
        return Err(OReplayError::UnsupportedStage(
            download.stage.type_name().to_string(),
        ));
    }
    let date = race_date(download).ok_or(OReplayError::MissingDate)?;
    let classes = download
        .classes
        .iter()
        .enumerate()
        .map(|(i, c)| class(i, c))
        .collect::<Result<Vec<_>, _>>()?;
    let any_split = classes.iter().any(|c| {
        c.results.iter().any(|r| {
            r.punches
                .iter()
                .any(|p| p.code != START_CODE && p.code != FINISH_CODE && p.time.is_some())
        })
    });
    if !any_split {
        return Err(OReplayError::NoSplits);
    }
    Ok(Event {
        name: download.name(),
        date,
        classes,
    })
}

/// Día de la carrera en la zona del evento: el de la salida de la etapa o, si no la trae, el de
/// la primera salida de un corredor.
fn race_date(download: &Download) -> Option<NaiveDate> {
    let zone = download
        .event
        .timezone
        .as_deref()
        .and_then(|z| Tz::from_str(z).ok())
        .unwrap_or(DEFAULT_TIME_ZONE);
    let instant = download.stage.start.or_else(|| {
        download
            .classes
            .iter()
            .flat_map(|c| &c.runners)
            .filter_map(|r| r.stage.as_ref()?.start_time)
            .min()
    })?;
    Some(instant.with_timezone(&zone).date_naive())
}

/// Picadas de un corredor en el orden del recorrido: código y hora, sin radios.
fn ordered_splits(class: &str, result: &ApiResult) -> Result<Vec<Punch>, OReplayError> {
    let mut splits: Vec<&ApiSplit> = result
        .splits
        .iter()
        .filter(|s| !s.is_intermediate)
        .collect();
    splits.sort_by_key(|s| s.order_number.unwrap_or(i64::MAX));
    splits
        .into_iter()
        .map(|s| {
            let station = s
                .control
                .as_ref()
                .and_then(|c| c.station.clone())
                .unwrap_or_default();
            let code = station
                .trim()
                .parse::<ControlCode>()
                .ok()
                .filter(|c| *c != START_CODE && *c != FINISH_CODE)
                .ok_or(OReplayError::InvalidStation {
                    class: class.to_string(),
                    station,
                })?;
            Ok(Punch {
                code,
                time: s.reading_time,
            })
        })
        .collect()
}

fn class(index: usize, c: &ClassResults) -> Result<Class, OReplayError> {
    let short_name = non_empty(c.class.short_name.as_deref()).map(str::to_string);
    let name = non_empty(c.class.long_name.as_deref())
        .map(str::to_string)
        .or_else(|| short_name.clone())
        .unwrap_or_else(|| format!("Categoría {}", index + 1));

    let runs = c
        .runners
        .iter()
        .map(|r| {
            r.stage
                .as_ref()
                .map(|s| ordered_splits(&name, s))
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let course = course(&c.runners, &runs);

    let results = c
        .runners
        .iter()
        .zip(&runs)
        .map(|(runner, splits)| result(runner, splits.as_deref(), &course))
        .collect();
    Ok(Class {
        // Posición en la descarga, desde 1: los id de OReplay son UUID.
        id: u32::try_from(index + 1).unwrap_or(u32::MAX),
        name,
        short_name,
        course,
        results,
    })
}

/// La secuencia de balizas más repetida entre los clasificados con picadas o, si no hay
/// ninguno, entre todos los que traen picadas. Con empate, la primera que aparece.
fn course(runners: &[ApiRunner], runs: &[Option<Vec<Punch>>]) -> Course {
    let sequences = |only_ok: bool| {
        runners
            .iter()
            .zip(runs)
            .filter(|(r, _)| !only_ok || status(r) == RaceStatus::Ok)
            .filter_map(|(_, run)| run.as_ref().filter(|p| !p.is_empty()))
            .map(|punches| punches.iter().map(|p| p.code).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    let mut candidates = sequences(true);
    if candidates.is_empty() {
        candidates = sequences(false);
    }
    let mut counts: HashMap<&[ControlCode], usize> = HashMap::new();
    for seq in &candidates {
        *counts.entry(seq.as_slice()).or_default() += 1;
    }
    let mut best: (&[ControlCode], usize) = (&[], 0);
    for seq in &candidates {
        let count = counts.get(seq.as_slice()).copied().unwrap_or_default();
        if count > best.1 {
            best = (seq, count);
        }
    }
    Course {
        controls: best.0.to_vec(),
    }
}

fn status(runner: &ApiRunner) -> RaceStatus {
    let Some(result) = &runner.stage else {
        return RaceStatus::DidNotStart;
    };
    match result.status_code.as_deref().map(str::trim) {
        Some("0") if !runner.is_nc && !result.is_nc => RaceStatus::Ok,
        Some("1") => RaceStatus::DidNotStart,
        _ => RaceStatus::NotClassified,
    }
}

fn result(runner: &ApiRunner, splits: Option<&[Punch]>, course: &Course) -> RaceResult {
    let status = status(runner);
    let stage = runner.stage.as_ref();
    let place = stage
        .and_then(|s| s.position)
        .filter(|_| status == RaceStatus::Ok)
        .and_then(|p| u16::try_from(p).ok())
        .filter(|p| *p > 0);

    // Cada baliza del recorrido toma la primera picada con su código que no se haya usado,
    // en orden: si el corredor tiene otra secuencia, lo que no casa queda sin hora.
    let splits = splits.unwrap_or_default();
    let mut next = 0;
    let mut punches = Vec::with_capacity(course.controls.len() + 2);
    punches.push(Punch {
        code: START_CODE,
        time: stage.and_then(|s| s.start_time),
    });
    for code in &course.controls {
        let found = splits[next..].iter().position(|p| p.code == *code);
        let time = found.and_then(|k| {
            next += k + 1;
            splits[next - 1].time
        });
        punches.push(Punch { code: *code, time });
    }
    punches.push(Punch {
        code: FINISH_CODE,
        time: stage.and_then(|s| s.finish_time),
    });

    RaceResult {
        runner: Runner {
            given_name: non_empty(runner.full_name.as_deref())
                .unwrap_or_default()
                .to_string(),
            family_name: String::new(),
            club: runner
                .club
                .as_ref()
                .and_then(|c| non_empty(c.short_name.as_deref()))
                .map(str::to_string),
            bib: positive(runner.bib_number.as_deref()),
            si_card: positive(runner.sicard.as_deref()),
            sex: match runner.sex.as_deref().map(str::trim) {
                Some("M") => Some(Sex::Male),
                Some("F") => Some(Sex::Female),
                _ => None,
            },
        },
        status,
        place,
        punches,
    }
}

fn non_empty(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|t| !t.is_empty())
}

fn positive(text: Option<&str>) -> Option<u32> {
    text?.trim().parse::<u32>().ok().filter(|n| *n > 0)
}
