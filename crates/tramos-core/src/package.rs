//! Paquete por carrera: la unidad de intercambio corredor → entrenadora (`docs/paquete.md`).
//!
//! Un JSON por corredor y carrera, con un nivel de permiso que decide qué lleva: el resumen, los
//! originales del recorrido (reducidos, sin dorsal, tarjeta ni sexo) y las etiquetas, o además el
//! track. Aquí están el formato, cómo se construye a partir de una carrera y cómo se lee y se
//! comprueba.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::identify::ResultRef;
use crate::lost_time::LostTimeConfig;
use crate::model::{Event, RaceStatus, Track};
use crate::race_format::RaceFormat;
use crate::runner_report::runner_report;
use crate::taxonomy::LegTag;

/// Valor de `format` en todos los paquetes.
pub const PACKAGE_FORMAT: &str = "tramos-paquete";

/// Versión del formato que escribe y entiende esta versión de Tramos.
pub const PACKAGE_VERSION: u32 = 1;

/// Qué comparte el corredor de una carrera. Cada nivel incluye lo del anterior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareLevel {
    /// Solo el resumen de la carrera.
    Aggregates,
    /// Además, los originales del recorrido y las etiquetas.
    Legs,
    /// Además, el track del reloj.
    Track,
}

/// Qué decide compartir el corredor de una carrera: nada (no se exporta) o un nivel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareChoice {
    /// No se comparte: no hay paquete.
    #[serde(rename = "none")]
    Nothing,
    Aggregates,
    Legs,
    Track,
}

impl ShareChoice {
    /// Todas, de menos a más.
    pub const ALL: [Self; 4] = [Self::Nothing, Self::Aggregates, Self::Legs, Self::Track];

    /// El nombre con el que se serializa (`none`, `aggregates`, `legs`, `track`).
    pub fn key(self) -> &'static str {
        match self {
            Self::Nothing => "none",
            Self::Aggregates => "aggregates",
            Self::Legs => "legs",
            Self::Track => "track",
        }
    }

    /// La elección con ese nombre ([`ShareChoice::key`]).
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.key() == key)
    }

    /// El nivel del paquete; `None` si no se comparte.
    pub fn level(self) -> Option<ShareLevel> {
        match self {
            Self::Nothing => None,
            Self::Aggregates => Some(ShareLevel::Aggregates),
            Self::Legs => Some(ShareLevel::Legs),
            Self::Track => Some(ShareLevel::Track),
        }
    }
}

/// La carrera del paquete.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageRace {
    /// Igual para todos los corredores de la carrera ([`race_id`]).
    pub race_id: String,
    pub date: NaiveDate,
    pub name: Option<String>,
    pub format: Option<RaceFormat>,
}

/// El corredor que exporta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageRunner {
    /// Generado una vez por base de datos; no sale del nombre ni de la tarjeta.
    pub runner_id: String,
    /// El nombre que el corredor escribió para sí mismo en la app.
    pub display_name: String,
}

/// Lo que vio el corredor de su carrera, calculado con sus umbrales.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaceSummary {
    pub class_name: String,
    pub status: RaceStatus,
    pub place: Option<u16>,
    pub total_s: Option<f64>,
    pub lost_time_s: Option<f64>,
    pub error_count: usize,
    pub time_without_errors_s: Option<f64>,
    pub usual_performance: Option<f64>,
    pub consistency: Option<f64>,
}

/// Originales del recorrido, reducidos ([`shared_course`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedCourse {
    /// Solo las categorías del recorrido del corredor: de cada uno, nombre, club y resultado.
    pub event: Event,
    /// El resultado del corredor dentro de `event`.
    pub result: ResultRef,
}

/// Etiqueta de un tramo del corredor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageTag {
    /// Desde 1.
    pub leg_index: usize,
    pub taxonomy_version: String,
    pub tag: LegTag,
}

/// Track del reloj del corredor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedTrack {
    pub track: Track,
    /// Desfase entre reloj y cronometraje fijado a mano; `None` = automático.
    pub manual_offset_s: Option<f64>,
}

/// Paquete de un corredor y una carrera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RacePackage {
    /// Siempre [`PACKAGE_FORMAT`].
    pub format: String,
    /// Versión del formato ([`PACKAGE_VERSION`] al escribirlo).
    pub version: u32,
    pub level: ShareLevel,
    pub exported_at: DateTime<Utc>,
    /// Versión del núcleo con la que se calculó `summary`.
    pub core_version: String,
    pub runner: PackageRunner,
    pub race: PackageRace,
    /// Umbrales con los que se calculó `summary`.
    pub config: LostTimeConfig,
    pub summary: RaceSummary,
    /// Desde `legs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub course: Option<SharedCourse>,
    /// Desde `legs`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<PackageTag>,
    /// Solo en `track`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<SharedTrack>,
}

/// Errores al construir o leer un paquete.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum PackageError {
    #[error("el paquete no es un JSON válido: {0}")]
    Json(String),
    #[error("no es un paquete de Tramos")]
    NotAPackage,
    #[error(
        "el paquete es de la versión {found} del formato y esta versión de Tramos solo entiende \
         hasta la {supported}: actualiza la app"
    )]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("el contenido del paquete no cuadra con su nivel de permiso")]
    LevelMismatch,
    #[error("el paquete apunta a un resultado que no está en el recorrido")]
    ResultOutOfRange,
    #[error("el resultado no aparece en la carrera o no se puede analizar")]
    NotAnalyzed,
    #[error("para compartir el track, la carrera tiene que tener track")]
    MissingTrack,
}

/// Identificador de una carrera, igual para todos los que la importaron: las 32 primeras cifras
/// hexadecimales del SHA-256 de la fecha y, por categoría en orden de nombre, su nombre y las
/// balizas de su recorrido. No depende de tiempos ni de nombres de corredores.
pub fn race_id(event: &Event) -> String {
    let mut classes: Vec<String> = event
        .classes
        .iter()
        .map(|c| {
            let controls: Vec<String> = c.course.controls.iter().map(u16::to_string).collect();
            format!("{}:{}", c.name, controls.join(","))
        })
        .collect();
    classes.sort();
    let mut hasher = Sha256::new();
    hasher.update(b"tramos-race-v1\n");
    hasher.update(event.date.format("%Y-%m-%d").to_string().as_bytes());
    for class in &classes {
        hasher.update(b"\n");
        hasher.update(class.as_bytes());
    }
    hex(&hasher.finalize()[..16])
}

/// Nombre del fichero de un paquete: el mismo para el mismo corredor y la misma carrera.
pub fn package_file_name(race_id: &str, runner_id: &str) -> String {
    format!("tramos-{race_id}-{runner_id}.json")
}

/// Bytes en hexadecimal, en minúsculas.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Los originales del recorrido del resultado `at`: solo las categorías con su mismo recorrido y,
/// de cada corredor (también el propio), nombre, apellidos, club, estado, puesto y picadas, lo
/// mismo que publican los resultados. Dorsal, tarjeta y sexo se quitan. `None` si `at` no está en
/// `event`.
pub fn shared_course(event: &Event, at: ResultRef) -> Option<SharedCourse> {
    let own_class = event.classes.get(at.class_index)?;
    at.get(event)?;
    let mut shared = Event {
        name: event.name.clone(),
        date: event.date,
        classes: Vec::new(),
    };
    let mut result = None;
    for (i, class) in event.classes.iter().enumerate() {
        if class.course != own_class.course {
            continue;
        }
        if i == at.class_index {
            result = Some(ResultRef {
                class_index: shared.classes.len(),
                result_index: at.result_index,
            });
        }
        let mut class = class.clone();
        for r in &mut class.results {
            r.runner.bib = None;
            r.runner.si_card = None;
            r.runner.sex = None;
        }
        shared.classes.push(class);
    }
    Some(SharedCourse {
        event: shared,
        result: result?,
    })
}

/// Lo necesario para construir un paquete.
#[derive(Debug, Clone)]
pub struct PackageInput<'a> {
    pub level: ShareLevel,
    pub exported_at: DateTime<Utc>,
    pub runner: PackageRunner,
    pub event: &'a Event,
    pub at: ResultRef,
    pub format: Option<RaceFormat>,
    pub config: LostTimeConfig,
    /// Se incluyen desde `legs`.
    pub tags: Vec<PackageTag>,
    /// Se incluye en `track`, que lo exige.
    pub track: Option<SharedTrack>,
}

/// Construye el paquete del resultado `input.at` con lo que permite su nivel.
pub fn build(input: PackageInput<'_>) -> Result<RacePackage, PackageError> {
    let report =
        runner_report(input.event, input.at, &input.config).ok_or(PackageError::NotAnalyzed)?;
    let (Some(class), Some(result)) = (
        input.event.classes.get(input.at.class_index),
        input.at.get(input.event),
    ) else {
        return Err(PackageError::NotAnalyzed);
    };
    let lost = &report.lost_time;
    let with_legs = input.level >= ShareLevel::Legs;
    let track = if input.level == ShareLevel::Track {
        Some(input.track.ok_or(PackageError::MissingTrack)?)
    } else {
        None
    };
    Ok(RacePackage {
        format: PACKAGE_FORMAT.to_string(),
        version: PACKAGE_VERSION,
        level: input.level,
        exported_at: input.exported_at,
        core_version: crate::VERSION.to_string(),
        runner: input.runner,
        race: PackageRace {
            race_id: race_id(input.event),
            date: input.event.date,
            name: input.event.name.clone(),
            format: input.format,
        },
        config: input.config,
        summary: RaceSummary {
            class_name: class.name.clone(),
            status: result.status,
            place: result.place,
            total_s: lost.total_s,
            lost_time_s: lost.lost_time_s,
            error_count: lost.error_count,
            time_without_errors_s: lost.time_without_errors_s,
            usual_performance: lost.usual_performance,
            consistency: lost.consistency,
        },
        course: if with_legs {
            Some(shared_course(input.event, input.at).ok_or(PackageError::NotAnalyzed)?)
        } else {
            None
        },
        tags: if with_legs { input.tags } else { Vec::new() },
        track,
    })
}

impl RacePackage {
    /// Lee un paquete y comprueba formato, versión y que el contenido cuadra con el nivel.
    pub fn parse(json: &str) -> Result<Self, PackageError> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| PackageError::Json(e.to_string()))?;
        if value.get("format").and_then(|f| f.as_str()) != Some(PACKAGE_FORMAT) {
            return Err(PackageError::NotAPackage);
        }
        // La versión, antes de leer el resto: una más nueva puede no encajar en estos tipos.
        let version = value
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .ok_or(PackageError::NotAPackage)?;
        if version > u64::from(PACKAGE_VERSION) {
            return Err(PackageError::UnsupportedVersion {
                found: u32::try_from(version).unwrap_or(u32::MAX),
                supported: PACKAGE_VERSION,
            });
        }
        let package: Self =
            serde_json::from_value(value).map_err(|e| PackageError::Json(e.to_string()))?;
        package.check()?;
        Ok(package)
    }

    /// El paquete en JSON.
    pub fn to_json(&self) -> Result<String, PackageError> {
        serde_json::to_string_pretty(self).map_err(|e| PackageError::Json(e.to_string()))
    }

    /// Nombre del fichero ([`package_file_name`]).
    pub fn file_name(&self) -> String {
        package_file_name(&self.race.race_id, &self.runner.runner_id)
    }

    fn check(&self) -> Result<(), PackageError> {
        let ok = match self.level {
            ShareLevel::Aggregates => {
                self.course.is_none() && self.track.is_none() && self.tags.is_empty()
            }
            ShareLevel::Legs => self.course.is_some() && self.track.is_none(),
            ShareLevel::Track => self.course.is_some() && self.track.is_some(),
        };
        if !ok {
            return Err(PackageError::LevelMismatch);
        }
        if let Some(course) = &self.course {
            if course.result.get(&course.event).is_none() {
                return Err(PackageError::ResultOutOfRange);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importers::spl;
    use crate::model::Class;
    use crate::model::TrackPoint;
    use crate::taxonomy::Confirmation;

    fn fixture() -> Event {
        let path = format!(
            "{}/../../fixtures/spl/baltanas-anon.spl",
            env!("CARGO_MANIFEST_DIR")
        );
        spl::read(&std::fs::read(path).unwrap()).unwrap()
    }

    /// El corredor 143 de M-SEN, como en el resto de tests del fixture.
    const AT: ResultRef = ResultRef {
        class_index: 9,
        result_index: 15,
    };

    fn input(event: &Event, level: ShareLevel) -> PackageInput<'_> {
        let t0: DateTime<Utc> = "2026-10-03T16:13:00Z".parse().unwrap();
        PackageInput {
            level,
            exported_at: "2026-10-05T10:00:00Z".parse().unwrap(),
            runner: PackageRunner {
                runner_id: "0123456789abcdef0123456789abcdef".into(),
                display_name: "Yo".into(),
            },
            event,
            at: AT,
            format: Some(RaceFormat::Sprint),
            config: LostTimeConfig::default(),
            tags: vec![PackageTag {
                leg_index: 9,
                taxonomy_version: "0".into(),
                tag: LegTag {
                    confirmation: Some(Confirmation::Error),
                    error_type: Some("navigation".into()),
                    ..LegTag::default()
                },
            }],
            track: Some(SharedTrack {
                track: Track {
                    sport: Some("running".into()),
                    points: vec![TrackPoint {
                        time: t0,
                        lat: 41.9,
                        lon: -4.2,
                        altitude_m: Some(800.0),
                        heart_rate_bpm: Some(150),
                        cadence_spm: None,
                        distance_m: Some(0.0),
                    }],
                },
                manual_offset_s: Some(7.0),
            }),
        }
    }

    #[test]
    fn each_level_round_trips_with_what_it_allows() {
        let event = fixture();
        for level in [ShareLevel::Aggregates, ShareLevel::Legs, ShareLevel::Track] {
            let package = build(input(&event, level)).unwrap();
            assert_eq!(package.level, level);
            assert_eq!(package.course.is_some(), level >= ShareLevel::Legs);
            assert_eq!(package.tags.len(), usize::from(level >= ShareLevel::Legs));
            assert_eq!(package.track.is_some(), level == ShareLevel::Track);
            let json = package.to_json().unwrap();
            assert_eq!(RacePackage::parse(&json).unwrap(), package, "{level:?}");
        }
    }

    #[test]
    fn the_summary_is_what_the_runner_sees() {
        let event = fixture();
        let package = build(input(&event, ShareLevel::Aggregates)).unwrap();
        let report = runner_report(&event, AT, &LostTimeConfig::default()).unwrap();
        assert_eq!(package.summary.class_name, "M-SEN");
        assert_eq!(package.summary.place, Some(16));
        assert_eq!(package.summary.lost_time_s, report.lost_time.lost_time_s);
        assert_eq!(package.summary.error_count, report.lost_time.error_count);
        assert_eq!(package.core_version, crate::VERSION);
        assert_eq!(package.race.race_id, race_id(&event));
        assert_eq!(
            package.file_name(),
            format!(
                "tramos-{}-0123456789abcdef0123456789abcdef.json",
                package.race.race_id
            )
        );
    }

    #[test]
    fn the_shared_course_keeps_names_and_clubs_only() {
        let event = fixture();
        let course = shared_course(&event, AT).unwrap();
        let own = &event.classes[AT.class_index];
        assert!(course.event.classes.iter().all(|c| c.course == own.course));
        assert!(course.event.classes.iter().any(|c| c.name == own.name));
        let originals: Vec<&Class> = event
            .classes
            .iter()
            .filter(|c| c.course == own.course)
            .collect();
        for (shared, original) in course.event.classes.iter().zip(&originals) {
            for (s, o) in shared.results.iter().zip(&original.results) {
                assert_eq!(
                    (&s.runner.given_name, &s.runner.family_name, &s.runner.club),
                    (&o.runner.given_name, &o.runner.family_name, &o.runner.club)
                );
                assert_eq!(
                    (s.runner.si_card, s.runner.bib, s.runner.sex),
                    (None, None, None)
                );
            }
        }
        let theirs = course.result.get(&course.event).unwrap();
        assert_eq!(
            theirs.runner.given_name,
            AT.get(&event).unwrap().runner.given_name
        );
        // La app de destino recalcula lo mismo con los originales reducidos.
        let config = LostTimeConfig::default();
        let original = runner_report(&event, AT, &config).unwrap();
        let recomputed = runner_report(&course.event, course.result, &config).unwrap();
        assert_eq!(recomputed.lost_time, original.lost_time);
    }

    #[test]
    fn race_id_depends_on_structure_not_on_times() {
        let event = fixture();
        let id = race_id(&event);
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));

        let mut retimed = event.clone();
        retimed.name = Some("Otro nombre".into());
        for p in retimed.classes[0].results[0].punches.iter_mut() {
            p.time = None;
        }
        retimed.classes.reverse();
        assert_eq!(race_id(&retimed), id);

        let mut other_day = event.clone();
        other_day.date = other_day.date.succ_opt().unwrap();
        assert_ne!(race_id(&other_day), id);
        let mut other_course = event.clone();
        other_course.classes[0].course.controls.push(250);
        assert_ne!(race_id(&other_course), id);
    }

    #[test]
    fn track_level_needs_a_track() {
        let event = fixture();
        let mut no_track = input(&event, ShareLevel::Track);
        no_track.track = None;
        assert_eq!(build(no_track), Err(PackageError::MissingTrack));
        // En los otros niveles el track no viaja aunque lo haya.
        assert!(
            build(input(&event, ShareLevel::Legs))
                .unwrap()
                .track
                .is_none()
        );
    }

    #[test]
    fn parse_rejects_what_is_not_a_valid_package() {
        let event = fixture();
        let package = build(input(&event, ShareLevel::Legs)).unwrap();
        let mut value = serde_json::to_value(&package).unwrap();

        assert!(matches!(
            RacePackage::parse("no es json"),
            Err(PackageError::Json(_))
        ));
        assert_eq!(
            RacePackage::parse(r#"{"format":"otra-cosa","version":1}"#),
            Err(PackageError::NotAPackage)
        );

        let mut newer = value.clone();
        newer["version"] = (PACKAGE_VERSION + 1).into();
        assert_eq!(
            RacePackage::parse(&newer.to_string()),
            Err(PackageError::UnsupportedVersion {
                found: PACKAGE_VERSION + 1,
                supported: PACKAGE_VERSION
            })
        );

        // Nivel que no cuadra: dice `aggregates` pero trae el recorrido.
        let mut mismatch = value.clone();
        mismatch["level"] = "aggregates".into();
        assert_eq!(
            RacePackage::parse(&mismatch.to_string()),
            Err(PackageError::LevelMismatch)
        );

        value["course"]["result"]["result_index"] = 9999.into();
        assert_eq!(
            RacePackage::parse(&value.to_string()),
            Err(PackageError::ResultOutOfRange)
        );
    }

    #[test]
    fn share_choice_reads_none_and_the_levels() {
        let choices: Vec<ShareChoice> =
            serde_json::from_str(r#"["none","aggregates","legs","track"]"#).unwrap();
        assert_eq!(
            choices.iter().map(|c| c.level()).collect::<Vec<_>>(),
            vec![
                None,
                Some(ShareLevel::Aggregates),
                Some(ShareLevel::Legs),
                Some(ShareLevel::Track)
            ]
        );
        for choice in ShareChoice::ALL {
            assert_eq!(
                serde_json::to_string(&choice).unwrap(),
                format!("\"{}\"", choice.key())
            );
            assert_eq!(ShareChoice::from_key(choice.key()), Some(choice));
        }
        assert_eq!(ShareChoice::from_key("todo"), None);
    }

    #[test]
    fn the_file_name_only_depends_on_race_and_runner() {
        let event = fixture();
        let package = build(input(&event, ShareLevel::Legs)).unwrap();
        assert_eq!(
            package.file_name(),
            package_file_name(&race_id(&event), &package.runner.runner_id)
        );
        assert_eq!(package_file_name("r", "c"), "tramos-r-c.json");
    }
}
