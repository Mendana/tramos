//! Modelo de dominio compartido por importadores, análisis y app.
//!
//! Convenciones (ver `docs/modelo.md`):
//! - Los instantes son absolutos, en `DateTime<Utc>`. La conversión desde la hora local de la
//!   carrera se hace en el importador.
//! - Las duraciones son segundos en `f64`.
//! - Las magnitudes llevan la unidad en el nombre del campo (`altitude_m`, `split_s`…).

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// Código numérico de una baliza, tal y como aparece en el cronometraje.
pub type ControlCode = u16;

/// Código especial de la salida en el cronometraje.
pub const START_CODE: ControlCode = 32736;

/// Código especial de la meta en el cronometraje.
pub const FINISH_CODE: ControlCode = 32752;

/// Una carrera: fecha, nombre y sus categorías.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Nombre de la prueba, si la fuente lo trae.
    pub name: Option<String>,
    /// Día de la carrera (fecha local).
    pub date: NaiveDate,
    pub classes: Vec<Class>,
}

/// Categoría: recorrido que corre y resultados de sus corredores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Class {
    /// Identificador dentro del fichero de origen.
    pub id: u32,
    pub name: String,
    pub short_name: Option<String>,
    pub course: Course,
    pub results: Vec<RaceResult>,
}

/// Recorrido: balizas en orden, sin la salida ni la meta.
///
/// Un recorrido con `n` balizas tiene `n + 1` tramos: salida → primera, …, última → meta.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Course {
    pub controls: Vec<ControlCode>,
}

/// Corredor tal y como aparece en una carrera. Nunca guarda la fecha de nacimiento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Runner {
    /// Valor numérico con el que la fuente abre el registro del corredor. **No es una clave
    /// única** ni identifica al corredor: en el .spl es el campo `0x80`, que resulta ser la
    /// longitud en bytes del resto del registro y se repite entre corredores, también dentro
    /// de una misma categoría (`docs/formato-spl.md`).
    ///
    /// Se conserva tal cual para no perder información del fichero, pero nada debe usarlo para
    /// identificar, agrupar ni buscar corredores. Un resultado dentro de una carrera se
    /// identifica por su posición: índice de la categoría en `Event::classes` e índice en
    /// `Class::results`. Entre carreras, por la persona a la que se vincula (`tramos-store`).
    pub id: u32,
    pub given_name: String,
    pub family_name: String,
    pub club: Option<String>,
    pub bib: Option<u32>,
    /// Número de tarjeta SportIdent.
    pub si_card: Option<u32>,
    pub sex: Option<Sex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sex {
    Male,
    Female,
}

/// Picada: baliza marcada y, si se registró, el instante.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Punch {
    pub code: ControlCode,
    pub time: Option<DateTime<Utc>>,
}

/// Resultado de un corredor en una categoría.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaceResult {
    pub runner: Runner,
    pub status: RaceStatus,
    /// Puesto en la categoría; solo los clasificados lo tienen.
    pub place: Option<u16>,
    /// Picadas en orden, desde la salida hasta la meta.
    pub punches: Vec<Punch>,
}

/// Estado del corredor en la clasificación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RaceStatus {
    /// Clasificado: el único estado que cuenta para la referencia del tiempo perdido.
    Ok,
    /// Tomó la salida pero no se clasificó (baliza fallida, abandono…).
    NotClassified,
    /// No presentado.
    DidNotStart,
    /// Código de estado que aún no sabemos interpretar; se conserva tal cual.
    Unknown(u8),
}

/// Tramo de un recorrido: de una baliza a la siguiente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Leg {
    /// Número del tramo, empezando en 1 (el que sale del triángulo de salida).
    pub index: usize,
    pub from: ControlCode,
    pub to: ControlCode,
    /// Tiempo del tramo en segundos; `None` si falta la picada de algún extremo.
    pub split_s: Option<f64>,
}

/// Registro del reloj del corredor: puntos ordenados por instante.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub points: Vec<TrackPoint>,
}

/// Punto del track. Solo existe si hay posición; el resto de magnitudes son opcionales.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackPoint {
    pub time: DateTime<Utc>,
    /// Latitud en grados (WGS84).
    pub lat: f64,
    /// Longitud en grados (WGS84).
    pub lon: f64,
    /// Altitud en metros.
    pub altitude_m: Option<f64>,
    /// Pulso en pulsaciones por minuto.
    pub heart_rate_bpm: Option<u8>,
    /// Cadencia en pasos por minuto, contando los dos pies.
    pub cadence_spm: Option<f64>,
    /// Distancia acumulada desde el inicio del registro, en metros.
    pub distance_m: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn utc(h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 3, h, m, s).unwrap()
    }

    fn sample_event() -> Event {
        let runner = Runner {
            id: 7,
            given_name: "Ana".into(),
            family_name: "Pérez".into(),
            club: Some("Club A".into()),
            bib: Some(101),
            si_card: Some(2_000_123),
            sex: Some(Sex::Female),
        };
        Event {
            name: Some("Chinchón".into()),
            date: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
            classes: vec![Class {
                id: 1,
                name: "F21A".into(),
                short_name: None,
                course: Course {
                    controls: vec![31, 45],
                },
                results: vec![RaceResult {
                    runner,
                    status: RaceStatus::Ok,
                    place: Some(1),
                    punches: vec![
                        Punch {
                            code: START_CODE,
                            time: Some(utc(9, 0, 0)),
                        },
                        Punch {
                            code: 31,
                            time: Some(utc(9, 1, 30)),
                        },
                        Punch {
                            code: 45,
                            time: None,
                        },
                        Punch {
                            code: FINISH_CODE,
                            time: Some(utc(9, 4, 0)),
                        },
                    ],
                }],
            }],
        }
    }

    #[test]
    fn event_round_trips_through_json() {
        let event = sample_event();
        let text = serde_json::to_string(&event).unwrap();
        let back: Event = serde_json::from_str(&text).unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn event_json_uses_iso_dates_and_snake_case() {
        let value = serde_json::to_value(sample_event()).unwrap();
        assert_eq!(value["date"], json!("2026-10-03"));

        let result = &value["classes"][0]["results"][0];
        assert_eq!(result["status"], json!("ok"));
        assert_eq!(result["runner"]["sex"], json!("female"));
        assert_eq!(result["punches"][0]["time"], json!("2026-10-03T09:00:00Z"));
        assert_eq!(result["punches"][2]["time"], json!(null));
    }

    #[test]
    fn runner_json_has_no_birthdate() {
        let value = serde_json::to_value(&sample_event().classes[0].results[0].runner).unwrap();
        let keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert!(!keys.iter().any(|k| k.contains("birth")), "{keys:?}");
    }

    #[test]
    fn unknown_status_keeps_its_code() {
        let status = RaceStatus::Unknown(6);
        let value = serde_json::to_value(status).unwrap();
        assert_eq!(value, json!({ "unknown": 6 }));
        assert_eq!(serde_json::from_value::<RaceStatus>(value).unwrap(), status);
    }

    #[test]
    fn leg_and_track_round_trip_through_json() {
        let leg = Leg {
            index: 1,
            from: START_CODE,
            to: 31,
            split_s: Some(90.0),
        };
        let track = Track {
            points: vec![TrackPoint {
                time: utc(9, 0, 0),
                lat: 40.1355,
                lon: -3.4227,
                altitude_m: Some(745.2),
                heart_rate_bpm: Some(152),
                cadence_spm: None,
                distance_m: Some(0.0),
            }],
        };
        let leg_back: Leg = serde_json::from_str(&serde_json::to_string(&leg).unwrap()).unwrap();
        let track_back: Track =
            serde_json::from_str(&serde_json::to_string(&track).unwrap()).unwrap();
        assert_eq!(leg_back, leg);
        assert_eq!(track_back, track);
    }
}
