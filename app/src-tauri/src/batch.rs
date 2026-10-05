//! Importar por lotes: una carpeta con los .spl y los FIT de carreras antiguas (#41).
//!
//! 1. Busca en la carpeta y sus subcarpetas los .spl y los .fit (por la extensión) y los lee.
//! 2. En cada .spl busca al corredor con la identidad de los ajustes (`tramos_core::identify`).
//!    Solo sigue si lo encuentra de forma única: en la duda no importa (no hay forma de borrar
//!    una carrera desde la app) y lo dice en el resumen.
//! 3. Empareja cada FIT con una carrera por fecha y hora ([`pair_fits`]).
//! 4. Importa cada carrera con [`import::import`], como la importación de una en una: reimportar
//!    no duplica y un FIT que no se alinea no se guarda.
//!
//! Todo es Rust sin Tauri, como [`crate::import`]. El criterio está en `docs/app.md`,
//! "Importar una carpeta".

use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use serde::Serialize;
use thiserror::Error;
use tramos_core::alignment::{AlignmentOptions, race_window};
use tramos_core::identify::{
    Identification, ResultRef, RunnerIdentity, identify_runner, normalize_name,
};
use tramos_core::importers::{fit, spl};
use tramos_core::model::Event;
use tramos_core::race_format::suggest_format;
use tramos_store::{Store, StoreError};

use crate::import::{self, ImportRequest};
use crate::settings;

/// Dos FIT con un solape que difiere menos que esto encajan igual de bien con una carrera.
const TIE_S: f64 = 1.0;

/// Errores que impiden importar la carpeta entera. Los de cada fichero van en el resumen.
#[derive(Debug, Error)]
pub enum BatchError {
    #[error("no se pudo leer la carpeta {path}: {source}")]
    ReadDir {
        path: String,
        source: std::io::Error,
    },
    #[error(
        "para importar una carpeta hace falta tu tarjeta SI o tu nombre en Ajustes: con ellos te \
         busco en cada carrera"
    )]
    NoIdentity,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Qué ha pasado con un .spl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchRaceStatus {
    /// Carrera nueva guardada.
    Imported,
    /// Ese .spl ya estaba importado: se ha reutilizado su carrera, sin duplicarla.
    AlreadyImported,
    /// No se ha guardado nada: `messages` dice por qué.
    NotImported,
}

/// Un .spl de la carpeta y lo que se ha hecho con él.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BatchRace {
    pub spl_path: String,
    /// `None` si el .spl no se ha podido leer.
    pub event_name: Option<String>,
    pub event_date: Option<NaiveDate>,
    pub status: BatchRaceStatus,
    /// Resultado del usuario, si se ha importado.
    pub result_id: Option<i64>,
    /// FIT emparejado con la carrera, si lo hay.
    pub fit: Option<PairedFit>,
    /// Avisos o el motivo de no importarla, en español.
    pub messages: Vec<String>,
}

/// FIT emparejado con una carrera.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PairedFit {
    pub path: String,
    /// Se ha alineado y guardado el track. Si no, `BatchRace::messages` dice por qué.
    pub track_saved: bool,
    pub offset_s: Option<f64>,
    pub confidence: Option<f64>,
}

/// Por qué un FIT no se ha emparejado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnpairedReason {
    /// Es un FIT válido, pero no coincide con ninguna carrera (o encajaba mejor otro).
    NoRace,
    /// No se puede leer o no tiene posiciones.
    Invalid,
    /// Es una copia exacta de otro FIT de la carpeta.
    Duplicate,
}

/// Un FIT que no se ha emparejado con ninguna carrera.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UnpairedFit {
    pub path: String,
    pub reason: UnpairedReason,
    /// La explicación, en español.
    pub message: String,
}

/// Resumen de la importación de una carpeta.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BatchSummary {
    /// Un elemento por .spl, de la carrera más antigua a la más reciente; los ilegibles, al final.
    pub races: Vec<BatchRace>,
    pub unpaired_fits: Vec<UnpairedFit>,
    /// Problemas de la carpeta (subcarpetas que no se pueden leer…), en español.
    pub warnings: Vec<String>,
}

/// Intervalo de tiempo en UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl Interval {
    /// Segundos en común con `other`; negativo si no se tocan.
    fn overlap_s(&self, other: &Interval) -> f64 {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        (end - start).as_seconds_f64()
    }
}

/// Con qué se queda una carrera al emparejar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RaceFit {
    /// Ningún FIT coincide con ella.
    None,
    /// El índice del FIT.
    Fit(usize),
    /// Varios FIT encajan igual de bien: no se elige ninguno.
    Ambiguous(Vec<usize>),
}

/// Qué ha sido de un FIT al emparejar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitFate {
    /// Emparejado con esa carrera.
    Paired(usize),
    /// Encajaba igual que otro con esa carrera, que se ha quedado sin FIT.
    Tied(usize),
    /// Coincidía con esa carrera, pero otro FIT encajaba mejor.
    Outmatched(usize),
    /// No coincide con ninguna carrera.
    NoRace,
}

/// Emparejamiento de los FIT con las carreras.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pairing {
    /// Una entrada por carrera, en el orden de `races`.
    pub races: Vec<RaceFit>,
    /// Una entrada por FIT, en el orden de `fits`.
    pub fits: Vec<FitFate>,
}

/// Empareja los FIT con las carreras por fecha y hora.
///
/// - La **ventana** de una carrera es la de la alineación (`docs/alineacion.md`): de la salida a
///   la meta del corredor, ampliada `margin_s` por cada lado (el desfase máximo de la
///   alineación). `None` si el corredor no tiene horas: esa carrera no se empareja.
/// - Un FIT **coincide** con una carrera si su intervalo (del primer al último punto) se solapa
///   con la ventana más de 0 s.
/// - Cada carrera se queda con el FIT que más se solapa con ella, y cada FIT va a una sola
///   carrera: se reparten de mayor a menor solape (con el mismo solape, primero la carrera y el
///   FIT que van antes).
/// - Si al tocarle a una carrera hay otro FIT libre que se solapa con ella lo mismo (menos de
///   [`TIE_S`] de diferencia), la carrera se queda **sin FIT** ([`RaceFit::Ambiguous`]): no se
///   adivina. Esos FIT pueden emparejarse aún con otra carrera.
pub fn pair_fits(races: &[Option<Interval>], fits: &[Interval], margin_s: f64) -> Pairing {
    let margin = TimeDelta::milliseconds((margin_s * 1000.0).round() as i64);
    let mut candidates: Vec<(f64, usize, usize)> = Vec::new();
    for (r, window) in races.iter().enumerate() {
        let Some(window) = window else { continue };
        let window = Interval {
            start: window.start - margin,
            end: window.end + margin,
        };
        for (f, interval) in fits.iter().enumerate() {
            let overlap = window.overlap_s(interval);
            if overlap > 0.0 {
                candidates.push((overlap, r, f));
            }
        }
    }
    candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));

    let mut race_fits = vec![RaceFit::None; races.len()];
    let mut fates = vec![FitFate::NoRace; fits.len()];
    let mut race_done = vec![false; races.len()];
    let mut fit_used = vec![false; fits.len()];
    for &(overlap, r, f) in &candidates {
        if race_done[r] || fit_used[f] {
            continue;
        }
        race_done[r] = true;
        let tied: Vec<usize> = candidates
            .iter()
            .filter(|&&(o, r2, f2)| {
                r2 == r && f2 != f && !fit_used[f2] && (overlap - o).abs() < TIE_S
            })
            .map(|&(_, _, f2)| f2)
            .collect();
        if tied.is_empty() {
            fit_used[f] = true;
            race_fits[r] = RaceFit::Fit(f);
            fates[f] = FitFate::Paired(r);
        } else {
            let mut all = vec![f];
            all.extend(tied);
            for &t in &all {
                fates[t] = FitFate::Tied(r);
            }
            race_fits[r] = RaceFit::Ambiguous(all);
        }
    }
    // Los que coincidían con alguna carrera pero se han quedado fuera: la de mayor solape.
    for &(_, r, f) in &candidates {
        if fates[f] == FitFate::NoRace {
            fates[f] = FitFate::Outmatched(r);
        }
    }
    Pairing {
        races: race_fits,
        fits: fates,
    }
}

/// Un .spl leído y con el corredor encontrado.
struct Race {
    path: String,
    bytes: Vec<u8>,
    event: Event,
    /// `None` si no se ha encontrado al corredor de forma única (va en `messages`).
    result: Option<ResultRef>,
    window: Option<Interval>,
    messages: Vec<String>,
}

/// Un FIT leído con posiciones.
struct Fit {
    path: String,
    interval: Interval,
    sport: Option<String>,
}

/// Importa todas las carreras de una carpeta (ver el módulo) y devuelve el resumen.
///
/// Solo falla si no se puede leer la carpeta, si no hay identidad en los ajustes o si falla la
/// base de datos al buscar; los problemas de cada fichero van en el resumen.
pub fn import_folder(store: &mut Store, folder: &str) -> Result<BatchSummary, BatchError> {
    let identity = settings::identity(store)?;
    let has_name = identity
        .full_name
        .as_deref()
        .is_some_and(|n| !normalize_name(n).is_empty());
    if identity.si_card.is_none() && !has_name {
        return Err(BatchError::NoIdentity);
    }
    let time_zone = settings::time_zone(store)?;

    let mut warnings = Vec::new();
    let mut spl_paths = Vec::new();
    let mut fit_paths = Vec::new();
    let root = Path::new(folder);
    let entries = std::fs::read_dir(root).map_err(|source| BatchError::ReadDir {
        path: folder.to_string(),
        source,
    })?;
    collect_files(entries, &mut spl_paths, &mut fit_paths, &mut warnings);
    spl_paths.sort();
    fit_paths.sort();

    // Los .spl: leerlos y buscar al corredor.
    let mut races: Vec<Race> = Vec::new();
    let mut summary_races: Vec<BatchRace> = Vec::new();
    for path in spl_paths {
        let Some(path) = path_text(&path, &mut warnings) else {
            continue;
        };
        let not_imported = |message: String| BatchRace {
            spl_path: path.clone(),
            event_name: None,
            event_date: None,
            status: BatchRaceStatus::NotImported,
            result_id: None,
            fit: None,
            messages: vec![message],
        };
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                summary_races.push(not_imported(format!("No se ha podido leer: {err}.")));
                continue;
            }
        };
        if let Some(original) = races.iter().find(|r| r.bytes == bytes) {
            summary_races.push(not_imported(format!(
                "Es una copia de {}: se importa una sola vez.",
                file_name(&original.path)
            )));
            continue;
        }
        let event = match spl::read_with_time_zone(&bytes, time_zone) {
            Ok(event) => event,
            Err(err) => {
                summary_races.push(not_imported(format!("El .spl no es válido: {err}.")));
                continue;
            }
        };
        let mut messages = Vec::new();
        let result = match identify_runner(&event, &identity) {
            Identification::Unique {
                candidate,
                name_mismatch: false,
            } => Some(candidate.result),
            Identification::Unique {
                name_mismatch: true,
                ..
            } => {
                messages.push(
                    "Tu tarjeta aparece con otro nombre: impórtala sola para comprobar que es tu \
                     resultado."
                        .to_string(),
                );
                None
            }
            Identification::Ambiguous { candidates } => {
                messages.push(format!(
                    "Hay {} resultados que podrían ser tuyos: impórtala sola para elegir el tuyo.",
                    candidates.len()
                ));
                None
            }
            Identification::NotFound => {
                messages.push(
                    "No te he encontrado por tarjeta ni por nombre: impórtala sola para elegir \
                     tu resultado."
                        .to_string(),
                );
                None
            }
        };
        let window = match result.and_then(|r| r.get(&event)).map(race_window) {
            Some(Ok(w)) => Some(Interval {
                start: w.start,
                end: w.finish,
            }),
            Some(Err(_)) => {
                messages.push(
                    "Tu resultado no tiene horas de salida y meta: no se le puede buscar el FIT."
                        .to_string(),
                );
                None
            }
            None => None,
        };
        races.push(Race {
            path,
            bytes,
            event,
            result,
            window,
            messages,
        });
    }

    // Los FIT: leerlos y quedarse con su intervalo.
    let mut fits: Vec<Fit> = Vec::new();
    let mut fit_bytes: Vec<Vec<u8>> = Vec::new();
    let mut unpaired_fits: Vec<UnpairedFit> = Vec::new();
    for path in fit_paths {
        let Some(path) = path_text(&path, &mut warnings) else {
            continue;
        };
        let invalid = |message: String| UnpairedFit {
            path: path.clone(),
            reason: UnpairedReason::Invalid,
            message,
        };
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                unpaired_fits.push(invalid(format!("No se ha podido leer: {err}.")));
                continue;
            }
        };
        if let Some(i) = fit_bytes.iter().position(|b| *b == bytes) {
            unpaired_fits.push(UnpairedFit {
                path: path.clone(),
                reason: UnpairedReason::Duplicate,
                message: format!("Es una copia de {}.", file_name(&fits[i].path)),
            });
            continue;
        }
        let track = match fit::read(&bytes) {
            Ok(track) => track,
            Err(err) => {
                unpaired_fits.push(invalid(format!("No es un FIT válido: {err}.")));
                continue;
            }
        };
        let (Some(first), Some(last)) = (track.points.first(), track.points.last()) else {
            unpaired_fits.push(invalid(
                "No tiene puntos con posición GPS: no se puede usar.".to_string(),
            ));
            continue;
        };
        fits.push(Fit {
            path,
            interval: Interval {
                start: first.time,
                end: last.time,
            },
            sport: track.sport,
        });
        fit_bytes.push(bytes);
    }

    // Emparejar.
    let windows: Vec<Option<Interval>> = races.iter().map(|r| r.window).collect();
    let intervals: Vec<Interval> = fits.iter().map(|f| f.interval).collect();
    let margin_s = f64::from(AlignmentOptions::default().max_offset_s);
    let pairing = pair_fits(&windows, &intervals, margin_s);
    for (fit, fate) in fits.iter().zip(&pairing.fits) {
        let message = match *fate {
            FitFate::Paired(_) => continue,
            FitFate::Tied(r) => format!(
                "Hay otro FIT de la misma hora que {}: no se ha elegido ninguno. Importa esa \
                 carrera sola con el que sea.",
                race_label(&races[r])
            ),
            FitFate::Outmatched(r) => format!(
                "Coincide con {}, pero otro FIT encajaba mejor.",
                race_label(&races[r])
            ),
            FitFate::NoRace => "No coincide en fecha y hora con ninguna carrera en la que te \
                                haya encontrado."
                .to_string(),
        };
        unpaired_fits.push(UnpairedFit {
            path: fit.path.clone(),
            reason: UnpairedReason::NoRace,
            message,
        });
    }

    // Importar, de la más antigua a la más reciente.
    let mut order: Vec<usize> = (0..races.len()).collect();
    order.sort_by(|&a, &b| {
        (races[a].event.date, &races[a].path).cmp(&(races[b].event.date, &races[b].path))
    });
    let mut imported = Vec::with_capacity(races.len());
    for r in order {
        let race = &races[r];
        let paired = match &pairing.races[r] {
            RaceFit::Fit(f) => Some(&fits[*f]),
            RaceFit::Ambiguous(_) | RaceFit::None => None,
        };
        let mut messages = race.messages.clone();
        if let RaceFit::Ambiguous(tied) = &pairing.races[r] {
            messages.push(format!(
                "Hay {} FIT de la misma hora: no se ha elegido ninguno.",
                tied.len()
            ));
        }
        imported.push(import_race(
            store,
            race,
            paired,
            identity.clone(),
            messages,
        )?);
    }
    imported.extend(summary_races);

    Ok(BatchSummary {
        races: imported,
        unpaired_fits,
        warnings,
    })
}

/// Importa una carrera con su FIT (si lo hay) y la resume.
fn import_race(
    store: &mut Store,
    race: &Race,
    fit: Option<&Fit>,
    identity: RunnerIdentity,
    mut messages: Vec<String>,
) -> Result<BatchRace, BatchError> {
    let mut summary = BatchRace {
        spl_path: race.path.clone(),
        event_name: race.event.name.clone(),
        event_date: Some(race.event.date),
        status: BatchRaceStatus::NotImported,
        result_id: None,
        fit: None,
        messages: Vec::new(),
    };
    let Some(result) = race.result else {
        summary.messages = messages;
        return Ok(summary);
    };
    let already = import::is_imported(store, &race.bytes)?;
    let request = ImportRequest {
        spl_path: race.path.clone(),
        fit_path: fit.map(|f| f.path.clone()),
        result,
        // El formato sugerido, solo en una carrera nueva: el de una ya guardada lo puede haber
        // corregido el usuario.
        format: if already {
            None
        } else {
            suggest_format(&race.event)
        },
        identity,
    };
    match import::import(store, &request) {
        Ok(outcome) => {
            summary.status = if outcome.already_imported {
                messages.push("Ya estaba importada: no se ha duplicado.".to_string());
                BatchRaceStatus::AlreadyImported
            } else {
                BatchRaceStatus::Imported
            };
            summary.result_id = Some(outcome.result_id);
            messages.extend(outcome.warnings);
            if let (Some(fit), Some(alignment)) = (fit, outcome.alignment) {
                if !fit::is_on_foot(fit.sport.as_deref()) {
                    messages.push(format!(
                        "El reloj grabó el FIT como «{}», no como carrera a pie: comprueba que es \
                         el de esta carrera.",
                        fit.sport.as_deref().unwrap_or_default()
                    ));
                }
                messages.extend(alignment.messages);
                summary.fit = Some(PairedFit {
                    path: fit.path.clone(),
                    track_saved: alignment.track_saved,
                    offset_s: alignment.offset_s,
                    confidence: alignment.confidence,
                });
            }
        }
        Err(err) => messages.push(format!("No se ha podido importar: {err}.")),
    }
    summary.messages = messages;
    Ok(summary)
}

/// Recorre la carpeta y sus subcarpetas (sin las ocultas ni seguir enlaces a carpetas) y apunta
/// los .spl y los .fit.
fn collect_files(
    entries: std::fs::ReadDir,
    spl: &mut Vec<PathBuf>,
    fit: &mut Vec<PathBuf>,
    warnings: &mut Vec<String>,
) {
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warnings.push(format!(
                    "No se ha podido leer un elemento de la carpeta: {err}."
                ));
                continue;
            }
        };
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            match std::fs::read_dir(&path) {
                Ok(sub) => collect_files(sub, spl, fit, warnings),
                Err(err) => warnings.push(format!(
                    "No se ha podido leer la carpeta {}: {err}.",
                    path.display()
                )),
            }
            continue;
        }
        let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase());
        match extension.as_deref() {
            Some("spl") => spl.push(path),
            Some("fit") => fit.push(path),
            _ => {}
        }
    }
}

/// La ruta como texto; si no es UTF-8 válido, un aviso y `None`.
fn path_text(path: &Path, warnings: &mut Vec<String>) -> Option<String> {
    let text = path.to_str().map(str::to_string);
    if text.is_none() {
        warnings.push(format!(
            "Se ha saltado {}: el nombre tiene caracteres que no se pueden leer.",
            path.display()
        ));
    }
    text
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned())
}

/// «Nombre de la carrera» (fecha), para los mensajes.
fn race_label(race: &Race) -> String {
    let name = race
        .event
        .name
        .clone()
        .unwrap_or_else(|| file_name(&race.path));
    format!("«{name}» ({})", race.event.date.format("%d/%m/%Y"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tramos_core::race_format::RaceFormat;

    use super::*;
    use crate::races::list_races;

    fn fixture(path: &str) -> PathBuf {
        PathBuf::from(format!(
            "{}/../../fixtures/{path}",
            env!("CARGO_MANIFEST_DIR")
        ))
    }

    /// Fechas OLE del .spl (días desde 1899-12-30).
    const OCT_3: f64 = 46298.0;
    const OCT_10: f64 = 46305.0;
    const DEC_12: f64 = 46368.0;

    /// El .spl anonimizado con otra fecha: las mismas horas locales otro día (como en
    /// `crates/tramos-core/tests/alignment.rs`). No hay escritor de .spl: se cambia el `f64` de
    /// la etiqueta `0x19` (`docs/formato-spl.md`).
    fn spl_on(ole: f64) -> Vec<u8> {
        let spl = std::fs::read(fixture("spl/baltanas-anon.spl")).unwrap();
        let mut pattern = vec![0x19];
        pattern.extend(OCT_3.to_le_bytes());
        let at = spl
            .windows(pattern.len())
            .position(|w| w == pattern.as_slice())
            .unwrap();
        let mut out = spl.clone();
        out[at + 1..at + 9].copy_from_slice(&ole.to_le_bytes());
        out
    }

    /// CRC de 16 bits del FIT (el de `tools/fit_sintetico.py`).
    fn fit_crc(data: &[u8]) -> u16 {
        const TABLE: [u16; 16] = [
            0x0000, 0xCC01, 0xD801, 0x1400, 0xF001, 0x3C00, 0x2800, 0xE401, 0xA001, 0x6C00, 0x7800,
            0xB401, 0x5000, 0x9C01, 0x8801, 0x4400,
        ];
        let mut crc = 0u16;
        for &byte in data {
            for nibble in [byte & 0xF, byte >> 4] {
                let tmp = TABLE[usize::from(crc & 0xF)];
                crc = ((crc >> 4) & 0x0FFF) ^ tmp ^ TABLE[usize::from(nibble)];
            }
        }
        crc
    }

    /// El FIT sintético con todos los `timestamp` (campo 253) desplazados `shift` y, si se
    /// pide, otro deporte en la sesión (código del perfil FIT: 0 genérico, 2 ciclismo). Recorre
    /// los mensajes como los escribe `tools/fit_sintetico.py` (sin tiempo comprimido ni campos
    /// de desarrollador) y recalcula el CRC final.
    fn fit_variant(shift: TimeDelta, sport: Option<u8>) -> Vec<u8> {
        let mut data = std::fs::read(fixture("fit/baltanas-sintetico.fit")).unwrap();
        let header_len = usize::from(data[0]);
        let end = header_len + u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
        let shift_s = u32::try_from(shift.num_seconds()).unwrap();
        let mut definitions: HashMap<u8, (u16, Vec<(u8, usize)>)> = HashMap::new();
        let mut i = header_len;
        while i < end {
            let header = data[i];
            i += 1;
            assert_eq!(
                header & 0xA0,
                0,
                "ni tiempo comprimido ni campos de desarrollador"
            );
            let local = header & 0x0F;
            if header & 0x40 != 0 {
                let global = u16::from_le_bytes([data[i + 2], data[i + 3]]);
                let n = usize::from(data[i + 4]);
                i += 5;
                let fields = (0..n)
                    .map(|k| (data[i + 3 * k], usize::from(data[i + 3 * k + 1])))
                    .collect();
                i += 3 * n;
                definitions.insert(local, (global, fields));
            } else {
                let (global, fields) = &definitions[&local];
                for &(number, size) in fields {
                    if number == 253 && size == 4 {
                        let t = u32::from_le_bytes(data[i..i + 4].try_into().unwrap());
                        data[i..i + 4].copy_from_slice(&(t + shift_s).to_le_bytes());
                    }
                    if let (18, 5, Some(code)) = (*global, number, sport) {
                        data[i] = code;
                    }
                    i += size;
                }
            }
        }
        let crc = fit_crc(&data[..end]);
        data[end..end + 2].copy_from_slice(&crc.to_le_bytes());
        data
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> String {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        path.to_str().unwrap().to_string()
    }

    /// Base con la identidad del corredor del FIT sintético (tarjeta 143) en los ajustes.
    fn store_with_identity() -> Store {
        let mut store = Store::open_in_memory().unwrap();
        let identity = RunnerIdentity {
            si_card: Some(143),
            full_name: Some("N143 Apellido143".into()),
        };
        settings::save_identity(&mut store, &identity, true).unwrap();
        store
    }

    fn folder(dir: &tempfile::TempDir) -> &str {
        dir.path().to_str().unwrap()
    }

    fn date(text: &str) -> Option<NaiveDate> {
        Some(text.parse().unwrap())
    }

    /// Tres carreras (3 y 10 de octubre en horario de verano y 12 de diciembre en el de
    /// invierno) con sus tres FIT, en subcarpetas y con nombres que no dicen de qué carrera son.
    fn three_races(dir: &Path) -> HashMap<&'static str, String> {
        let day = TimeDelta::days(1);
        // El 12 de diciembre es UTC+1: las mismas horas locales caen 1 h más tarde en UTC.
        let december = day * 70 + TimeDelta::hours(1);
        HashMap::from([
            ("spl_oct_3", write(dir, "baltanas.spl", &spl_on(OCT_3))),
            ("spl_oct_10", write(dir, "2026/otra.SPL", &spl_on(OCT_10))),
            (
                "spl_dec_12",
                write(dir, "2026/dic/ultima.spl", &spl_on(DEC_12)),
            ),
            (
                "fit_dec_12",
                write(dir, "reloj/a.fit", &fit_variant(december, None)),
            ),
            (
                "fit_oct_3",
                write(dir, "reloj/b.FIT", &fit_variant(TimeDelta::zero(), None)),
            ),
            (
                "fit_oct_10",
                write(dir, "c.fit", &fit_variant(day * 7, None)),
            ),
        ])
    }

    #[test]
    fn three_races_are_imported_and_paired() {
        let dir = tempfile::tempdir().unwrap();
        let files = three_races(dir.path());
        write(dir.path(), "notas.txt", b"no es de ninguna carrera");
        let mut store = store_with_identity();

        let summary = import_folder(&mut store, folder(&dir)).unwrap();
        assert!(
            summary.unpaired_fits.is_empty(),
            "{:?}",
            summary.unpaired_fits
        );
        assert!(summary.warnings.is_empty(), "{:?}", summary.warnings);
        let found: Vec<_> = summary
            .races
            .iter()
            .map(|r| {
                let fit = r.fit.as_ref().unwrap();
                assert!(fit.track_saved, "{r:?}");
                assert!(fit.offset_s.unwrap().abs() < 2.0, "{r:?}");
                assert!(fit.confidence.unwrap() > 0.8, "{r:?}");
                assert!(r.messages.is_empty(), "{r:?}");
                assert!(r.result_id.is_some());
                (
                    r.status,
                    r.event_date,
                    r.spl_path.as_str(),
                    fit.path.as_str(),
                )
            })
            .collect();
        let imported = BatchRaceStatus::Imported;
        assert_eq!(
            found,
            [
                (
                    imported,
                    date("2026-10-03"),
                    &*files["spl_oct_3"],
                    &*files["fit_oct_3"]
                ),
                (
                    imported,
                    date("2026-10-10"),
                    &*files["spl_oct_10"],
                    &*files["fit_oct_10"]
                ),
                (
                    imported,
                    date("2026-12-12"),
                    &*files["spl_dec_12"],
                    &*files["fit_dec_12"]
                ),
            ]
        );

        let races = list_races(&store).unwrap();
        assert_eq!(races.len(), 3);
        assert!(races.iter().all(|r| r.has_track && r.class_name == "M-SEN"));
        assert!(races.iter().all(|r| r.format == Some(RaceFormat::Sprint)));
        assert_eq!(store.people().unwrap().len(), 1);
    }

    #[test]
    fn importing_the_folder_again_does_not_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        three_races(dir.path());
        let mut store = store_with_identity();
        let first = import_folder(&mut store, folder(&dir)).unwrap();
        // El usuario corrige el formato de una: reimportar no lo pisa.
        let corrected = first.races[0].result_id.unwrap();
        crate::races::set_race_format(&mut store, corrected, Some(RaceFormat::Middle)).unwrap();

        let again = import_folder(&mut store, folder(&dir)).unwrap();
        assert_eq!(again.races.len(), 3);
        for (before, after) in first.races.iter().zip(&again.races) {
            assert_eq!(after.status, BatchRaceStatus::AlreadyImported);
            assert_eq!(after.result_id, before.result_id);
            assert!(after.fit.as_ref().unwrap().track_saved);
            assert_eq!(after.messages, ["Ya estaba importada: no se ha duplicado."]);
        }
        let races = list_races(&store).unwrap();
        assert_eq!(races.len(), 3);
        assert!(races.iter().all(|r| r.has_track));
        let format_of = |id| races.iter().find(|r| r.result_id == id).unwrap().format;
        assert_eq!(format_of(corrected), Some(RaceFormat::Middle));
        assert_eq!(store.people().unwrap().len(), 1);
    }

    #[test]
    fn unpaired_and_invalid_files_go_to_the_summary() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let spl = write(d, "baltanas.spl", &spl_on(OCT_3));
        let copy = write(d, "copia/baltanas.spl", &spl_on(OCT_3));
        let broken_spl = write(d, "roto.spl", b"spl4 pero no");
        let fit = write(d, "carrera.fit", &fit_variant(TimeDelta::zero(), None));
        // Diez minutos tarde: también coincide con la carrera, pero menos.
        let late = write(d, "tarde.fit", &fit_variant(TimeDelta::minutes(10), None));
        let other_day = write(d, "rodaje.fit", &fit_variant(TimeDelta::days(3), None));
        let fit_copy = write(d, "z/copia.fit", &fit_variant(TimeDelta::zero(), None));
        let broken_fit = write(d, "roto.fit", b"no es un FIT");
        let mut store = store_with_identity();

        let summary = import_folder(&mut store, folder(&dir)).unwrap();
        let races: Vec<_> = summary
            .races
            .iter()
            .map(|r| {
                let fit = r.fit.as_ref().map(|f| f.path.as_str());
                (r.spl_path.as_str(), r.status, fit)
            })
            .collect();
        assert_eq!(
            races,
            [
                (&*spl, BatchRaceStatus::Imported, Some(&*fit)),
                (&*copy, BatchRaceStatus::NotImported, None),
                (&*broken_spl, BatchRaceStatus::NotImported, None),
            ]
        );
        assert_eq!(
            summary.races[1].messages,
            ["Es una copia de baltanas.spl: se importa una sola vez."]
        );
        assert!(summary.races[2].messages[0].starts_with("El .spl no es válido"));

        let fits: HashMap<&str, (UnpairedReason, &str)> = summary
            .unpaired_fits
            .iter()
            .map(|f| (f.path.as_str(), (f.reason, f.message.as_str())))
            .collect();
        assert_eq!(fits.len(), 4, "{fits:?}");
        assert_eq!(
            fits[&*late],
            (
                UnpairedReason::NoRace,
                "Coincide con «Cto. SPRINT Liga Norte/Liga FOCYL Baltanas» (03/10/2026), pero \
                 otro FIT encajaba mejor."
            )
        );
        assert_eq!(
            fits[&*other_day],
            (
                UnpairedReason::NoRace,
                "No coincide en fecha y hora con ninguna carrera en la que te haya encontrado."
            )
        );
        assert_eq!(
            fits[&*fit_copy],
            (UnpairedReason::Duplicate, "Es una copia de carrera.fit.")
        );
        assert_eq!(fits[&*broken_fit].0, UnpairedReason::Invalid);
        assert!(fits[&*broken_fit].1.starts_with("No es un FIT válido"));
        assert_eq!(list_races(&store).unwrap().len(), 1);
    }

    #[test]
    fn a_race_without_the_runner_is_not_imported() {
        let dir = tempfile::tempdir().unwrap();
        let spl = write(dir.path(), "baltanas.spl", &spl_on(OCT_3));
        let fit = write(
            dir.path(),
            "reloj.fit",
            &fit_variant(TimeDelta::zero(), None),
        );
        let mut store = Store::open_in_memory().unwrap();
        let nobody = RunnerIdentity {
            si_card: Some(999_999),
            full_name: Some("Nadie Ninguno".into()),
        };
        settings::save_identity(&mut store, &nobody, true).unwrap();

        let summary = import_folder(&mut store, folder(&dir)).unwrap();
        assert_eq!(summary.races.len(), 1);
        let race = &summary.races[0];
        assert_eq!(
            (race.spl_path.as_str(), race.status, race.result_id),
            (&*spl, BatchRaceStatus::NotImported, None)
        );
        assert_eq!(race.event_date, date("2026-10-03"));
        assert!(race.messages[0].starts_with("No te he encontrado"));
        assert_eq!(summary.unpaired_fits.len(), 1);
        assert_eq!(summary.unpaired_fits[0].path, fit);
        assert_eq!(summary.unpaired_fits[0].reason, UnpairedReason::NoRace);
        assert!(list_races(&store).unwrap().is_empty());
        assert!(store.people().unwrap().is_empty());
    }

    #[test]
    fn a_card_with_another_name_is_not_imported() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "baltanas.spl", &spl_on(OCT_3));
        let mut store = Store::open_in_memory().unwrap();
        let other_name = RunnerIdentity {
            si_card: Some(143),
            full_name: Some("Otro Nombre".into()),
        };
        settings::save_identity(&mut store, &other_name, true).unwrap();
        let summary = import_folder(&mut store, folder(&dir)).unwrap();
        assert_eq!(summary.races[0].status, BatchRaceStatus::NotImported);
        assert!(summary.races[0].messages[0].contains("otro nombre"));
        assert!(list_races(&store).unwrap().is_empty());
    }

    #[test]
    fn a_fit_from_another_sport_is_imported_with_a_warning() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "baltanas.spl", &spl_on(OCT_3));
        write(
            dir.path(),
            "bici.fit",
            &fit_variant(TimeDelta::zero(), Some(2)),
        );
        let mut store = store_with_identity();
        let summary = import_folder(&mut store, folder(&dir)).unwrap();
        let race = &summary.races[0];
        assert_eq!(race.status, BatchRaceStatus::Imported);
        assert!(race.fit.as_ref().unwrap().track_saved);
        assert_eq!(
            race.messages,
            [
                "El reloj grabó el FIT como «cycling», no como carrera a pie: comprueba que es el \
                 de esta carrera."
            ]
        );
    }

    #[test]
    fn two_fits_of_the_same_time_are_not_guessed() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "baltanas.spl", &spl_on(OCT_3));
        // Las mismas horas y otro contenido (otro deporte a pie): no es una copia.
        write(dir.path(), "a.fit", &fit_variant(TimeDelta::zero(), None));
        write(
            dir.path(),
            "b.fit",
            &fit_variant(TimeDelta::zero(), Some(0)),
        );
        let mut store = store_with_identity();
        let summary = import_folder(&mut store, folder(&dir)).unwrap();
        let race = &summary.races[0];
        assert_eq!(race.status, BatchRaceStatus::Imported);
        assert_eq!(race.fit, None);
        assert_eq!(
            race.messages,
            ["Hay 2 FIT de la misma hora: no se ha elegido ninguno."]
        );
        assert_eq!(summary.unpaired_fits.len(), 2);
        assert!(summary.unpaired_fits.iter().all(|f| {
            f.reason == UnpairedReason::NoRace && f.message.starts_with("Hay otro FIT")
        }));
        assert!(!list_races(&store).unwrap()[0].has_track);
    }

    #[test]
    fn needs_an_identity_and_a_readable_folder() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open_in_memory().unwrap();
        assert!(matches!(
            import_folder(&mut store, folder(&dir)),
            Err(BatchError::NoIdentity)
        ));
        // Un nombre que se queda vacío al normalizar tampoco vale.
        let blank = RunnerIdentity {
            si_card: None,
            full_name: Some(" - ".into()),
        };
        settings::save_identity(&mut store, &blank, true).unwrap();
        assert!(matches!(
            import_folder(&mut store, folder(&dir)),
            Err(BatchError::NoIdentity)
        ));

        let mut store = store_with_identity();
        let missing = dir.path().join("no-existe");
        let err = import_folder(&mut store, missing.to_str().unwrap()).unwrap_err();
        assert!(
            err.to_string().starts_with("no se pudo leer la carpeta"),
            "{err}"
        );
        let empty = import_folder(&mut store, folder(&dir)).unwrap();
        assert!(empty.races.is_empty() && empty.unpaired_fits.is_empty());
    }

    fn at(hh_mm: &str) -> DateTime<Utc> {
        format!("2026-10-03T{hh_mm}:00Z").parse().unwrap()
    }

    fn interval(start: &str, end: &str) -> Interval {
        Interval {
            start: at(start),
            end: at(end),
        }
    }

    #[test]
    fn pairs_each_fit_with_the_race_it_overlaps_most() {
        let races = [
            Some(interval("10:00", "10:30")),
            Some(interval("12:00", "12:30")),
            None,
        ];
        let fits = [
            interval("11:55", "12:40"),
            interval("09:58", "10:31"),
            interval("15:00", "16:00"),
            // Se solapa con la primera, pero menos que el anterior.
            interval("10:20", "11:00"),
        ];
        let pairing = pair_fits(&races, &fits, 60.0);
        assert_eq!(
            pairing.races,
            [RaceFit::Fit(1), RaceFit::Fit(0), RaceFit::None]
        );
        assert_eq!(
            pairing.fits,
            [
                FitFate::Paired(1),
                FitFate::Paired(0),
                FitFate::NoRace,
                FitFate::Outmatched(0)
            ]
        );
    }

    #[test]
    fn the_margin_is_the_alignment_offset() {
        // Empieza 30 s después de la meta: solo coincide gracias al margen.
        let races = [Some(interval("10:00", "10:30"))];
        let fits = [Interval {
            start: at("10:30") + TimeDelta::seconds(30),
            end: at("11:00"),
        }];
        assert_eq!(pair_fits(&races, &fits, 60.0).races, [RaceFit::Fit(0)]);
        let without_margin = pair_fits(&races, &fits, 0.0);
        assert_eq!(without_margin.races, [RaceFit::None]);
        assert_eq!(without_margin.fits, [FitFate::NoRace]);
    }

    #[test]
    fn each_fit_goes_to_one_race_and_ties_are_not_guessed() {
        let races = [
            Some(interval("10:00", "10:30")),
            Some(interval("10:40", "11:30")),
        ];
        // Los dos cubren la primera entera; el segundo, además, la segunda: con ella se solapa
        // 50 min, así que va primero y se lo queda. La primera se queda con el otro.
        let fits = [interval("09:50", "10:35"), interval("09:55", "11:30")];
        let pairing = pair_fits(&races, &fits, 0.0);
        assert_eq!(pairing.races, [RaceFit::Fit(0), RaceFit::Fit(1)]);

        // Dos que cubren la carrera entera: empate.
        let same = [interval("09:50", "10:35"), interval("09:55", "10:40")];
        let pairing = pair_fits(&races[..1], &same, 0.0);
        assert_eq!(pairing.races, [RaceFit::Ambiguous(vec![0, 1])]);
        assert_eq!(pairing.fits, [FitFate::Tied(0), FitFate::Tied(0)]);
    }
}
