//! Descargar una carrera de OReplay pegando su enlace (#185).
//!
//! Se pide a la API pública de OReplay el evento, las categorías de la etapa y los resultados de
//! cada categoría (las URL y la lectura de las respuestas son del núcleo,
//! `tramos_core::importers::oreplay`), se junta todo en una descarga y se guarda en el
//! directorio de datos de la app. Desde ahí se importa como un .spl: [`crate::import`] distingue
//! los dos por el contenido. La descarga va en Rust, así que no toca la CSP de la interfaz.
//!
//! Si el enlace es el del evento y tiene varias etapas individuales, no se descarga nada: se
//! devuelven para que el usuario elija. Flujo en `docs/app.md`, "Importar".

use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use thiserror::Error;
use tramos_core::importers::oreplay::{
    self, ApiStage, ClassResults, Download, OReplayError, classes_url, event_url, parse_classes,
    parse_event, parse_results, parse_url, results_url,
};

/// Subdirectorio del directorio de datos de la app con las descargas.
pub const DOWNLOAD_DIR: &str = "oreplay";
/// Categorías que se piden a la vez.
const CONCURRENT_REQUESTS: usize = 4;
/// Tiempo máximo de cada petición.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Errores al descargar. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum DownloadError {
    #[error(transparent)]
    OReplay(#[from] OReplayError),
    #[error("no se pudo descargar de OReplay: {0}")]
    Network(String),
    #[error("el evento no tiene carreras individuales (solo relevos, score o totales)")]
    NoClassicStage,
    #[error("esa etapa no es de este evento o no es una carrera individual")]
    UnknownStage,
    #[error("la etapa no tiene categorías")]
    NoClasses,
    #[error("no se pudo guardar la descarga en {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
}

/// Etapa que el usuario puede elegir.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StageChoice {
    pub id: String,
    pub name: String,
}

/// Lo que devuelve la descarga.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DownloadOutcome {
    /// Guardada en `path`, lista para importar como un .spl.
    Downloaded {
        path: String,
        name: Option<String>,
        classes: usize,
        runners: usize,
    },
    /// El enlace es el del evento y hay varias etapas individuales: hay que elegir una y volver a
    /// llamar con ella.
    ChooseStage { stages: Vec<StageChoice> },
}

/// Quien hace las peticiones: [`Http`] en la app, uno de mentira en los tests.
pub trait Fetch: Clone + Send + Sync + 'static {
    /// Cuerpo de la respuesta a un GET, o el error en español.
    fn get(&self, url: String) -> impl Future<Output = Result<Vec<u8>, String>> + Send;
}

/// Peticiones HTTPS de verdad.
#[derive(Clone)]
pub struct Http {
    client: reqwest::Client,
}

impl Http {
    pub fn new() -> Result<Self, DownloadError> {
        // Mismo proveedor de cifrado que el actualizador (`tauri-plugin-updater`); instalarlo dos
        // veces solo devuelve error, que no importa.
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
        let client = reqwest::Client::builder()
            .user_agent(concat!("Tramos/", env!("CARGO_PKG_VERSION")))
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| DownloadError::Network(e.to_string()))?;
        Ok(Self { client })
    }
}

impl Fetch for Http {
    async fn get(&self, url: String) -> Result<Vec<u8>, String> {
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("OReplay responde {status}"));
        }
        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| e.to_string())
    }
}

/// Descarga la etapa del enlace (o `stage_id`, si el usuario la ha elegido) y la guarda en `dir`.
pub async fn download<F: Fetch>(
    fetch: &F,
    url: &str,
    stage_id: Option<&str>,
    dir: &Path,
) -> Result<DownloadOutcome, DownloadError> {
    let link = parse_url(url)?;
    let event = parse_event(&get(fetch, event_url(&link.event_id)).await?)?;

    let classic: Vec<&ApiStage> = event.classic_stages().collect();
    let wanted = stage_id.map(str::to_string).or(link.stage_id);
    let stage = match wanted {
        Some(id) => classic
            .iter()
            .find(|s| s.id.eq_ignore_ascii_case(&id))
            .copied()
            .ok_or(DownloadError::UnknownStage)?,
        None => match classic.as_slice() {
            [] => return Err(DownloadError::NoClassicStage),
            [only] => *only,
            several => {
                return Ok(DownloadOutcome::ChooseStage {
                    stages: several
                        .iter()
                        .enumerate()
                        .map(|(i, s)| StageChoice {
                            id: s.id.clone(),
                            name: stage_name(s, i),
                        })
                        .collect(),
                });
            }
        },
    }
    .clone();

    let classes = parse_classes(&get(fetch, classes_url(&event.id, &stage.id)).await?)?;
    if classes.is_empty() {
        return Err(DownloadError::NoClasses);
    }
    let mut results = Vec::with_capacity(classes.len());
    for chunk in classes.chunks(CONCURRENT_REQUESTS) {
        let handles: Vec<_> = chunk
            .iter()
            .map(|class| {
                let fetch = fetch.clone();
                let url = results_url(&event.id, &stage.id, &class.id);
                tauri::async_runtime::spawn(async move { fetch.get(url).await })
            })
            .collect();
        for (class, handle) in chunk.iter().zip(handles) {
            let body = handle
                .await
                .map_err(|e| DownloadError::Network(e.to_string()))?
                .map_err(DownloadError::Network)?;
            results.push(ClassResults {
                class: class.clone(),
                runners: parse_results(&body)?,
            });
        }
    }

    let download = Download::new(event, stage, results);
    // Que se pueda importar: si no, mejor decirlo ahora que al revisar.
    let parsed = oreplay::to_event(&download)?;
    let name = download.name();
    let path = dir.join(file_name(name.as_deref()));
    let write_error = |source| DownloadError::Write {
        path: path.display().to_string(),
        source,
    };
    std::fs::create_dir_all(dir).map_err(write_error)?;
    std::fs::write(&path, download.to_json()).map_err(write_error)?;
    Ok(DownloadOutcome::Downloaded {
        path: path.display().to_string(),
        name,
        classes: parsed.classes.len(),
        runners: parsed.classes.iter().map(|c| c.results.len()).sum(),
    })
}

async fn get<F: Fetch>(fetch: &F, url: String) -> Result<Vec<u8>, DownloadError> {
    fetch.get(url).await.map_err(DownloadError::Network)
}

fn stage_name(stage: &ApiStage, index: usize) -> String {
    let name = stage
        .description
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Etapa {}", index + 1));
    match stage.start {
        Some(start) => format!("{name} ({})", start.format("%Y-%m-%d")),
        None => name,
    }
}

/// Nombre del fichero: el de la carrera sin caracteres que no valgan en Windows.
fn file_name(name: Option<&str>) -> PathBuf {
    let clean: String = name
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') {
                c
            } else {
                ' '
            }
        })
        .collect();
    let clean = clean
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches('.')
        .chars()
        .take(80)
        .collect::<String>();
    let clean = if clean.is_empty() {
        "carrera".to_string()
    } else {
        clean
    };
    PathBuf::from(format!("{clean} (OReplay).json"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use serde_json::{Value, json};

    use super::*;

    const EVENT: &str = "0fb98d3c-d506-4406-95c2-f28cb1863889";
    const STAGE_A: &str = "557abbec-9bb7-4cea-9c45-c0634aedf22e";
    const STAGE_B: &str = "99b4b528-3d13-4194-b884-1dfad8a03a79";

    /// Respuestas por URL; una URL que no está da error, como un 404.
    #[derive(Clone, Default)]
    struct Fake(Arc<HashMap<String, Vec<u8>>>);

    impl Fetch for Fake {
        async fn get(&self, url: String) -> Result<Vec<u8>, String> {
            self.0
                .get(&url)
                .cloned()
                .ok_or_else(|| format!("OReplay responde 404 en {url}"))
        }
    }

    fn stage(id: &str, name: &str, kind: &str) -> Value {
        json!({"id": id, "description": name, "start": "2026-03-07T09:00:00.000+00:00",
               "stage_type": {"description": kind}})
    }

    fn runner(name: &str, card: u32) -> Value {
        json!({"full_name": name, "sicard": card.to_string(), "sex": "F", "is_nc": false,
               "stage": {"start_time": "2026-03-07T09:00:00.000+00:00",
                         "finish_time": "2026-03-07T09:20:00.000+00:00",
                         "status_code": "0", "position": 1, "is_nc": false,
                         "splits": [
                            {"order_number": 1, "is_intermediate": false,
                             "reading_time": "2026-03-07T09:05:00.000+00:00",
                             "control": {"station": "31"}},
                            {"order_number": 2, "is_intermediate": false,
                             "reading_time": "2026-03-07T09:12:00.000+00:00",
                             "control": {"station": "45"}}]}})
    }

    fn fake(stages: Vec<Value>) -> Fake {
        let mut map = HashMap::new();
        let body = |v: Value| serde_json::to_vec(&v).unwrap();
        map.insert(
            event_url(EVENT),
            body(
                json!({"data": {"id": EVENT, "description": "Trofeo: «prueba»/2026",
                                 "timezone": "Europe/Madrid", "stages": stages}}),
            ),
        );
        for stage in [STAGE_A, STAGE_B] {
            map.insert(
                classes_url(EVENT, stage),
                body(json!({"data": [
                    {"id": "c1", "short_name": "F21", "long_name": "F21"},
                    {"id": "c2", "short_name": "M21", "long_name": "M21"}]})),
            );
            map.insert(
                results_url(EVENT, stage, "c1"),
                body(json!({"data": [runner("Ana Pérez", 1), runner("Eva Ruiz", 2)]})),
            );
            map.insert(
                results_url(EVENT, stage, "c2"),
                body(json!({"data": [runner("Luis Gómez", 3)]})),
            );
        }
        Fake(Arc::new(map))
    }

    fn run(
        fake: &Fake,
        url: &str,
        stage: Option<&str>,
        dir: &Path,
    ) -> Result<DownloadOutcome, DownloadError> {
        tauri::async_runtime::block_on(download(fake, url, stage, dir))
    }

    fn event_link() -> String {
        format!("https://www.oreplay.es/competitions/{EVENT}")
    }

    #[test]
    fn a_stage_link_downloads_and_saves_an_importable_file() {
        let fake = fake(vec![
            stage(STAGE_A, "Media", "Classic"),
            stage(STAGE_B, "Larga", "Classic"),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let outcome = run(
            &fake,
            &format!("{}/{STAGE_B}", event_link()),
            None,
            dir.path(),
        )
        .unwrap();
        let DownloadOutcome::Downloaded {
            path,
            name,
            classes,
            runners,
        } = outcome
        else {
            panic!("se esperaba la descarga: {outcome:?}");
        };
        assert_eq!(name.as_deref(), Some("Trofeo: «prueba»/2026 · Larga"));
        assert_eq!((classes, runners), (2, 3));
        assert_eq!(
            Path::new(&path).file_name().unwrap(),
            "Trofeo prueba 2026 Larga (OReplay).json"
        );
        let event = oreplay::read(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(event.classes[0].results[1].runner.given_name, "Eva Ruiz");
        assert_eq!(event.classes[1].course.controls, [31, 45]);
    }

    #[test]
    fn an_event_link_with_several_stages_asks_which_one() {
        let fake = fake(vec![
            stage(STAGE_A, "Media", "Classic"),
            stage("00000000-0000-0000-0000-000000000000", "Relevos", "Relay"),
            stage(STAGE_B, "", "Classic"),
        ]);
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            run(&fake, &event_link(), None, dir.path()).unwrap(),
            DownloadOutcome::ChooseStage {
                stages: vec![
                    StageChoice {
                        id: STAGE_A.to_string(),
                        name: "Media (2026-03-07)".to_string()
                    },
                    StageChoice {
                        id: STAGE_B.to_string(),
                        name: "Etapa 2 (2026-03-07)".to_string()
                    },
                ]
            }
        );
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
        // Con la etapa elegida, se descarga.
        assert!(matches!(
            run(&fake, &event_link(), Some(STAGE_A), dir.path()),
            Ok(DownloadOutcome::Downloaded { .. })
        ));
    }

    #[test]
    fn a_single_classic_stage_needs_no_choice() {
        let fake = fake(vec![
            stage(STAGE_A, "Media", "Classic"),
            stage(STAGE_B, "Score", "Score"),
        ]);
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            run(&fake, &event_link(), None, dir.path()),
            Ok(DownloadOutcome::Downloaded { .. })
        ));
    }

    #[test]
    fn errors_are_explained() {
        let dir = tempfile::tempdir().unwrap();
        let relay_only = fake(vec![stage(STAGE_A, "Relevos", "Relay")]);
        assert!(matches!(
            run(&relay_only, &event_link(), None, dir.path()),
            Err(DownloadError::NoClassicStage)
        ));
        assert!(matches!(
            run(
                &relay_only,
                &format!("{}/{STAGE_A}", event_link()),
                None,
                dir.path()
            ),
            Err(DownloadError::UnknownStage)
        ));
        assert!(matches!(
            run(&relay_only, "https://example.com/x", None, dir.path()),
            Err(DownloadError::OReplay(OReplayError::InvalidUrl(_)))
        ));
        let missing = Fake::default();
        let err = run(&missing, &event_link(), None, dir.path()).unwrap_err();
        assert!(matches!(err, DownloadError::Network(_)));
        assert!(err.to_string().contains("404"), "{err}");
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(
            file_name(Some("  A/B\\C:D*E?F\"G<H>I|J  ")),
            PathBuf::from("A B C D E F G H I J (OReplay).json")
        );
        assert_eq!(file_name(None), PathBuf::from("carrera (OReplay).json"));
        assert_eq!(
            file_name(Some("...")),
            PathBuf::from("carrera (OReplay).json")
        );
    }
}
