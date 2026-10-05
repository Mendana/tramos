//! Lector de ficheros FIT de actividad (reloj del corredor).
//!
//! Decodifica el fichero con `fitparser` y convierte los mensajes `record` en un [`Track`]. El
//! formato y las decisiones de importación están en `docs/formato-fit.md`. Resumen:
//!
//! - Solo se leen los mensajes `record`; el resto (eventos, vueltas, sesión, mensajes
//!   propietarios o desconocidos) se ignora.
//! - Un `record` sin posición válida o sin instante no produce punto (`docs/modelo.md`).
//! - Posición en grados (el FIT la guarda en semicírculos), instante en UTC.
//! - Altitud: `enhanced_altitude` si está; si no, `altitude`.
//! - Deporte: el de la sesión (`session.sport`) o, si no hay sesión, el del mensaje `sport`.
//! - Cadencia en pasos/min con los dos pies: `2 × (cadence + fractional_cadence)` en deportes a
//!   pie o sin deporte, porque ahí el FIT guarda las zancadas de un pie por minuto. En los demás
//!   (bici…) es `cadence + fractional_cadence` tal cual, en rpm.
//! - Puntos ordenados por instante (orden estable: los empates conservan el orden del fichero).

use chrono::{DateTime, Utc};
use fitparser::de::{DecodeOption, FitObject, FitStreamProcessor};
use fitparser::profile::MesgNum;
use fitparser::{FitDataRecord, Value};
use thiserror::Error;

use crate::model::{Track, TrackPoint};

/// Grados por semicírculo: el FIT reparte 180° en 2^31 semicírculos.
const DEGREES_PER_SEMICIRCLE: f64 = 180.0 / 2_147_483_648.0;

/// Deportes a pie (nombres del perfil FIT): su cadencia es de un pie y se duplica.
const FOOT_SPORTS: [&str; 5] = ["generic", "running", "walking", "hiking", "mountaineering"];

/// Errores al leer un FIT.
#[derive(Debug, Error)]
pub enum FitError {
    /// `fitparser` no ha podido decodificar el fichero: cabecera o CRC inválidos, fichero
    /// truncado, mensaje de datos sin su definición…
    #[error("no se puede decodificar el FIT: {0}")]
    Decode(#[from] fitparser::Error),
}

/// Lee un FIT de actividad completo y devuelve su track.
///
/// Un FIT sin ningún `record` con posición (por ejemplo, una actividad en cinta) da un
/// [`Track`] vacío, no un error.
pub fn read(data: &[u8]) -> Result<Track, FitError> {
    let record = MesgNum::Record.as_u16();
    // Además de los `record`, la sesión y el deporte: dicen cómo leer la cadencia.
    let wanted = [record, MesgNum::Session.as_u16(), MesgNum::Sport.as_u16()];
    let mut processor = FitStreamProcessor::new();
    // Los campos que no están en el perfil FIT (muchos son propietarios de Garmin) se descartan.
    processor.add_option(DecodeOption::DropUnknownFields);

    let mut records = Vec::new();
    let mut input = data;
    while !input.is_empty() {
        let (rest, object) = processor.deserialize_next(input)?;
        match object {
            // Fin de un fichero; puede venir otro encadenado detrás.
            FitObject::Crc(_) => processor.reset(),
            FitObject::DataMessage(message) => {
                let number = message.global_message_number();
                let is_record = number == record;
                let is_wanted = wanted.contains(&number);
                // Todos los mensajes se decodifican, aunque se descarten, porque cualquiera con
                // `timestamp` fija la referencia de las cabeceras de tiempo comprimido.
                match processor.decode_message(message) {
                    Ok(decoded) if is_wanted => records.push(decoded),
                    Err(err) if is_record => return Err(err.into()),
                    // Un mensaje que no es `record` y que `fitparser` no sabe convertir (un
                    // campo propietario con un tipo inesperado) no afecta al track.
                    Ok(_) | Err(_) => {}
                }
            }
            FitObject::Header(_) | FitObject::DefinitionMessage(_) => {}
        }
        input = rest;
    }
    Ok(track_from_messages(&records))
}

/// Si el deporte del FIT (`Track::sport`) es a pie. Sin deporte se supone a pie, que es lo normal
/// en orientación (`docs/formato-fit.md`, "Deporte").
pub fn is_on_foot(sport: Option<&str>) -> bool {
    sport.is_none_or(|s| FOOT_SPORTS.contains(&s))
}

/// Convierte los mensajes decodificados en un track: un punto por cada `record` con posición
/// e instante, ordenados por instante, y el deporte de la actividad.
fn track_from_messages(messages: &[FitDataRecord]) -> Track {
    let sport = sport_from_messages(messages);
    let on_foot = is_on_foot(sport.as_deref());
    let mut points: Vec<TrackPoint> = messages
        .iter()
        .filter(|m| m.kind() == MesgNum::Record)
        .filter_map(|m| point_from_record(m, on_foot))
        .collect();
    points.sort_by_key(|p| p.time);
    Track { points, sport }
}

/// Deporte de la primera sesión que lo traiga; si ninguna, el del mensaje `sport`.
fn sport_from_messages(messages: &[FitDataRecord]) -> Option<String> {
    let from = |kind: MesgNum| {
        messages
            .iter()
            .filter(|m| m.kind() == kind)
            .find_map(|m| RecordFields(m).text("sport"))
    };
    from(MesgNum::Session).or_else(|| from(MesgNum::Sport))
}

/// Convierte un mensaje `record` en un punto, o `None` si le falta el instante o la posición.
/// `on_foot` duplica la cadencia (ver el comentario del módulo).
fn point_from_record(record: &FitDataRecord, on_foot: bool) -> Option<TrackPoint> {
    let fields = RecordFields(record);
    let time = fields.timestamp()?;
    let lat = fields.degrees("position_lat").filter(|v| v.abs() <= 90.0)?;
    let lon = fields
        .degrees("position_long")
        .filter(|v| v.abs() <= 180.0)?;

    let altitude_m = fields
        .number("enhanced_altitude")
        .or_else(|| fields.number("altitude"));
    let heart_rate_bpm = fields
        .number("heart_rate")
        .filter(|v| (0.0..=f64::from(u8::MAX)).contains(v))
        .map(|v| v.round() as u8);
    let feet = if on_foot { 2.0 } else { 1.0 };
    let cadence_spm = fields
        .number("cadence")
        .map(|rpm| feet * (rpm + fields.number("fractional_cadence").unwrap_or(0.0)));
    let distance_m = fields.number("distance");

    Some(TrackPoint {
        time,
        lat,
        lon,
        altitude_m,
        heart_rate_bpm,
        cadence_spm,
        distance_m,
    })
}

/// Acceso por nombre a los campos de un mensaje decodificado por `fitparser`, que ya aplica
/// la escala y el desplazamiento del perfil (metros, ppm, rpm…) salvo en la posición.
struct RecordFields<'a>(&'a FitDataRecord);

impl RecordFields<'_> {
    fn get(&self, name: &str) -> Option<&fitparser::FitDataField> {
        self.0.fields().iter().find(|f| f.name() == name)
    }

    fn timestamp(&self) -> Option<DateTime<Utc>> {
        match self.get("timestamp")?.value() {
            // `fitparser` devuelve el instante en la zona del sistema; el instante es el mismo.
            Value::Timestamp(t) => Some(t.with_timezone(&Utc)),
            _ => None,
        }
    }

    /// Texto del campo: el nombre del valor si `fitparser` lo conoce (un enum del perfil, como
    /// `running`) o el número tal cual si no.
    fn text(&self, name: &str) -> Option<String> {
        match self.get(name)?.value() {
            Value::String(s) => Some(s.clone()),
            Value::Enum(n) => Some(n.to_string()),
            _ => None,
        }
    }

    /// Valor numérico finito del campo, ya en las unidades del perfil.
    fn number(&self, name: &str) -> Option<f64> {
        let value = match self.get(name)?.value() {
            Value::Byte(v) | Value::Enum(v) | Value::UInt8(v) | Value::UInt8z(v) => f64::from(*v),
            Value::SInt8(v) => f64::from(*v),
            Value::SInt16(v) => f64::from(*v),
            Value::UInt16(v) | Value::UInt16z(v) => f64::from(*v),
            Value::SInt32(v) => f64::from(*v),
            Value::UInt32(v) | Value::UInt32z(v) => f64::from(*v),
            Value::SInt64(v) => *v as f64,
            Value::UInt64(v) | Value::UInt64z(v) => *v as f64,
            Value::Float32(v) => f64::from(*v),
            Value::Float64(v) => *v,
            Value::Timestamp(_) | Value::String(_) | Value::Array(_) | Value::Invalid => {
                return None;
            }
        };
        value.is_finite().then_some(value)
    }

    /// Coordenada en grados. El perfil FIT la da en semicírculos; si `fitparser` la devolviera
    /// ya en grados (unidad `deg` o `degrees`), se usa tal cual.
    fn degrees(&self, name: &str) -> Option<f64> {
        let value = self.number(name)?;
        match self.get(name)?.units() {
            "deg" | "degrees" => Some(value),
            _ => Some(value * DEGREES_PER_SEMICIRCLE),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};
    use fitparser::FitDataField;

    /// Punto de un `record` de carrera a pie.
    fn point(record: &FitDataRecord) -> Option<TrackPoint> {
        point_from_record(record, true)
    }

    fn field(name: &str, number: u8, value: Value, units: &str) -> FitDataField {
        FitDataField::new(name.into(), number, None, value, units.into())
    }

    fn timestamp(seconds: u32) -> FitDataField {
        let t = Utc.with_ymd_and_hms(2026, 10, 3, 16, 12, seconds).unwrap();
        field(
            "timestamp",
            253,
            Value::Timestamp(t.with_timezone(&Local)),
            "s",
        )
    }

    /// 2^30 semicírculos = 90°; 2^29 = 45°.
    fn position(lat_sc: i32, lon_sc: i32) -> Vec<FitDataField> {
        vec![
            field("position_lat", 0, Value::SInt32(lat_sc), "semicircles"),
            field("position_long", 1, Value::SInt32(lon_sc), "semicircles"),
        ]
    }

    fn record(fields: Vec<FitDataField>) -> FitDataRecord {
        let mut r = FitDataRecord::new(MesgNum::Record);
        r.extend(fields);
        r
    }

    #[test]
    fn converts_semicircles_to_degrees() {
        let mut fields = vec![timestamp(0)];
        fields.extend(position(1 << 29, -(1 << 30)));
        let p = point(&record(fields)).unwrap();
        assert_eq!(p.lat, 45.0);
        assert_eq!(p.lon, -90.0);
        assert_eq!(p.time.to_rfc3339(), "2026-10-03T16:12:00+00:00");
        assert_eq!(p.altitude_m, None);
        assert_eq!(p.heart_rate_bpm, None);
        assert_eq!(p.cadence_spm, None);
        assert_eq!(p.distance_m, None);
    }

    #[test]
    fn accepts_positions_already_in_degrees() {
        let fields = vec![
            timestamp(0),
            field("position_lat", 0, Value::Float64(41.938), "deg"),
            field("position_long", 1, Value::Float64(-4.249), "degrees"),
        ];
        let p = point(&record(fields)).unwrap();
        assert_eq!((p.lat, p.lon), (41.938, -4.249));
    }

    #[test]
    fn record_without_position_or_time_gives_no_point() {
        let no_position = record(vec![
            timestamp(0),
            field("heart_rate", 3, Value::UInt8(150), "bpm"),
        ]);
        let only_lat = record(vec![
            timestamp(0),
            field("position_lat", 0, Value::SInt32(1 << 29), "semicircles"),
        ]);
        let no_time = record(position(1 << 29, 1 << 29));
        assert_eq!(point(&no_position), None);
        assert_eq!(point(&only_lat), None);
        assert_eq!(point(&no_time), None);
    }

    #[test]
    fn out_of_range_position_gives_no_point() {
        // 2^30 + 1 semicírculos es algo más de 90° de latitud.
        let mut fields = vec![timestamp(0)];
        fields.extend(position((1 << 30) + 1, 0));
        assert_eq!(point(&record(fields)), None);
    }

    #[test]
    fn cadence_counts_both_feet_and_adds_fractional_part() {
        let mut fields = vec![timestamp(0)];
        fields.extend(position(0, 0));
        fields.push(field("cadence", 4, Value::UInt8(88), "rpm"));
        let p = point(&record(fields.clone())).unwrap();
        assert_eq!(p.cadence_spm, Some(176.0));

        fields.push(field("fractional_cadence", 53, Value::Float64(0.5), "rpm"));
        let p = point(&record(fields)).unwrap();
        assert_eq!(p.cadence_spm, Some(177.0));
    }

    #[test]
    fn fractional_cadence_alone_is_not_a_cadence() {
        let mut fields = vec![timestamp(0)];
        fields.extend(position(0, 0));
        fields.push(field("fractional_cadence", 53, Value::Float64(0.5), "rpm"));
        assert_eq!(point(&record(fields)).unwrap().cadence_spm, None);
    }

    fn with_cadence(seconds: u32, rpm: u8) -> FitDataRecord {
        let mut fields = vec![timestamp(seconds)];
        fields.extend(position(0, 0));
        fields.push(field("cadence", 4, Value::UInt8(rpm), "rpm"));
        record(fields)
    }

    fn sport_message(kind: MesgNum, sport: Value) -> FitDataRecord {
        let mut m = FitDataRecord::new(kind);
        m.push(field("sport", 5, sport, ""));
        m
    }

    #[test]
    fn cycling_cadence_is_not_doubled() {
        // La sesión va al final del fichero, detrás de los `record`.
        let messages = vec![
            with_cadence(0, 85),
            sport_message(MesgNum::Session, Value::String("cycling".into())),
        ];
        let track = track_from_messages(&messages);
        assert_eq!(track.sport.as_deref(), Some("cycling"));
        assert_eq!(track.points[0].cadence_spm, Some(85.0));
    }

    #[test]
    fn session_sport_wins_over_sport_message() {
        let messages = vec![
            sport_message(MesgNum::Sport, Value::String("cycling".into())),
            with_cadence(0, 88),
            sport_message(MesgNum::Session, Value::String("running".into())),
        ];
        let track = track_from_messages(&messages);
        assert_eq!(track.sport.as_deref(), Some("running"));
        assert_eq!(track.points[0].cadence_spm, Some(176.0));

        let only_sport = vec![
            sport_message(MesgNum::Sport, Value::String("hiking".into())),
            with_cadence(0, 50),
        ];
        let track = track_from_messages(&only_sport);
        assert_eq!(track.sport.as_deref(), Some("hiking"));
        assert_eq!(track.points[0].cadence_spm, Some(100.0));
    }

    #[test]
    fn unknown_or_missing_sport() {
        // Sin deporte se supone a pie.
        let track = track_from_messages(&[with_cadence(0, 88)]);
        assert_eq!(track.sport, None);
        assert_eq!(track.points[0].cadence_spm, Some(176.0));

        // Un enum que `fitparser` no sabe nombrar se guarda como número y no es a pie.
        let messages = vec![
            with_cadence(0, 70),
            sport_message(MesgNum::Session, Value::Enum(250)),
        ];
        let track = track_from_messages(&messages);
        assert_eq!(track.sport.as_deref(), Some("250"));
        assert_eq!(track.points[0].cadence_spm, Some(70.0));
    }

    #[test]
    fn on_foot_sports() {
        for sport in [None, Some("running"), Some("generic"), Some("hiking")] {
            assert!(is_on_foot(sport), "{sport:?}");
        }
        for sport in [Some("cycling"), Some("swimming"), Some("250")] {
            assert!(!is_on_foot(sport), "{sport:?}");
        }
    }

    #[test]
    fn prefers_enhanced_altitude() {
        let mut fields = vec![timestamp(0)];
        fields.extend(position(0, 0));
        fields.push(field("altitude", 2, Value::Float64(700.0), "m"));
        let p = point(&record(fields.clone())).unwrap();
        assert_eq!(p.altitude_m, Some(700.0));

        fields.push(field("enhanced_altitude", 78, Value::Float64(789.4), "m"));
        let p = point(&record(fields)).unwrap();
        assert_eq!(p.altitude_m, Some(789.4));
    }

    #[test]
    fn reads_heart_rate_and_distance() {
        let mut fields = vec![timestamp(0)];
        fields.extend(position(0, 0));
        fields.push(field("heart_rate", 3, Value::UInt8(152), "bpm"));
        fields.push(field("distance", 5, Value::Float64(1581.47), "m"));
        let p = point(&record(fields)).unwrap();
        assert_eq!(p.heart_rate_bpm, Some(152));
        assert_eq!(p.distance_m, Some(1581.47));
    }

    #[test]
    fn track_keeps_only_records_with_position_sorted_by_time() {
        let at = |s: u32, lat_sc: i32| {
            let mut fields = vec![timestamp(s)];
            fields.extend(position(lat_sc, 0));
            record(fields)
        };
        let mut lap = FitDataRecord::new(MesgNum::Lap);
        lap.push(timestamp(9));
        let mut unknown = FitDataRecord::new(MesgNum::Value(499));
        unknown.extend(position(1, 1));
        let messages = vec![
            at(2, 2),
            lap,
            record(vec![timestamp(1)]),
            at(0, 0),
            unknown,
            at(1, 1),
        ];
        let track = track_from_messages(&messages);
        let seconds: Vec<u32> = track
            .points
            .iter()
            .map(|p| p.time.timestamp() as u32 % 60)
            .collect();
        assert_eq!(seconds, [0, 1, 2]);
    }

    #[test]
    fn garbage_is_a_decode_error() {
        let err = read(b"esto no es un FIT").unwrap_err();
        assert!(matches!(err, FitError::Decode(_)), "{err}");
    }

    // --- FIT construido a mano, para probar `read` de punta a punta ---------------------------

    /// CRC de 16 bits del protocolo FIT (CRC-16/ARC).
    fn crc16(data: &[u8]) -> u16 {
        const TABLE: [u16; 16] = [
            0x0000, 0xCC01, 0xD801, 0x1400, 0xF001, 0x3C00, 0x2800, 0xE401, 0xA001, 0x6C00, 0x7800,
            0xB401, 0x5000, 0x9C01, 0x8801, 0x4400,
        ];
        data.iter().fold(0, |mut crc, &byte| {
            for nibble in [byte & 0xF, byte >> 4] {
                let tmp = TABLE[usize::from(crc & 0xF)];
                crc = ((crc >> 4) & 0x0FFF) ^ tmp ^ TABLE[usize::from(nibble)];
            }
            crc
        })
    }

    /// Codificador FIT mínimo (little endian, cabecera de 14 bytes).
    #[derive(Default)]
    struct FitBuilder {
        data: Vec<u8>,
    }

    impl FitBuilder {
        /// Mensaje de definición: campos `(número, tamaño, tipo base)`.
        fn define(&mut self, local: u8, global: u16, fields: &[(u8, u8, u8)]) -> &mut Self {
            self.data.extend([0x40 | local, 0, 0]);
            self.data.extend(global.to_le_bytes());
            self.data.push(fields.len() as u8);
            for &(number, size, base) in fields {
                self.data.extend([number, size, base]);
            }
            self
        }

        fn message(&mut self, local: u8, bytes: &[u8]) -> &mut Self {
            self.data.push(local);
            self.data.extend(bytes);
            self
        }

        fn finish(&self) -> Vec<u8> {
            let mut out = vec![14, 0x20];
            out.extend(2184u16.to_le_bytes());
            out.extend((self.data.len() as u32).to_le_bytes());
            out.extend(b".FIT");
            out.extend(crc16(&out).to_le_bytes());
            out.extend(&self.data);
            out.extend(crc16(&out).to_le_bytes());
            out
        }
    }

    /// Segundos desde la época FIT (1989-12-31T00:00:00Z) hasta 2026-10-03T16:12:`s`Z.
    fn fit_time(s: u32) -> [u8; 4] {
        let t = Utc.with_ymd_and_hms(2026, 10, 3, 16, 12, s).unwrap();
        ((t.timestamp() - 631_065_600) as u32).to_le_bytes()
    }

    fn record_bytes(s: u32, lat_sc: i32, lon_sc: i32, hr: u8, cadence: u8, frac: u8) -> Vec<u8> {
        let mut b = fit_time(s).to_vec();
        b.extend(lat_sc.to_le_bytes());
        b.extend(lon_sc.to_le_bytes());
        b.extend([hr, cadence, frac, 7]);
        b
    }

    #[test]
    fn reads_hand_built_fit_and_ignores_other_messages() {
        const UINT8: u8 = 0x02;
        const SINT32: u8 = 0x85;
        const UINT32: u8 = 0x86;
        const STRING: u8 = 0x07;
        let invalid = i32::MAX; // 0x7FFFFFFF: posición no válida
        let mut fit = FitBuilder::default();
        // record: timestamp, lat, lon, heart_rate, cadence, fractional_cadence (escala 128) y
        // un campo que no está en el perfil (200).
        fit.define(
            0,
            20,
            &[
                (253, 4, UINT32),
                (0, 4, SINT32),
                (1, 4, SINT32),
                (3, 1, UINT8),
                (4, 1, UINT8),
                (53, 1, UINT8),
                (200, 1, UINT8),
            ],
        )
        .message(0, &record_bytes(2, 1 << 29, -(1 << 29), 150, 88, 64))
        .message(0, &record_bytes(1, invalid, invalid, 149, 87, 0))
        // Mensaje desconocido (499), como los propietarios de Garmin.
        .define(1, 499, &[(0, 4, UINT32), (1, 1, UINT8)])
        .message(1, &[1, 2, 3, 4, 5])
        // session con `sport` (enum) escrito como texto: `fitparser` no sabe convertirlo.
        .define(2, 18, &[(253, 4, UINT32), (5, 3, STRING)])
        .message(2, &[fit_time(3).as_slice(), b"abc"].concat())
        .message(0, &record_bytes(0, 0, 1 << 30, 0xFF, 0xFF, 0xFF));

        let bytes = fit.finish();
        // `fitparser::from_bytes` falla en todo el fichero por culpa de la `session`.
        assert!(fitparser::from_bytes(&bytes).is_err());
        let track = read(&bytes).unwrap();
        assert_eq!(track.points.len(), 2);

        let first = &track.points[0];
        assert_eq!(first.time.to_rfc3339(), "2026-10-03T16:12:00+00:00");
        assert_eq!((first.lat, first.lon), (0.0, 90.0));
        // 0xFF es "sin valor" en un uint8.
        assert_eq!(first.heart_rate_bpm, None);
        assert_eq!(first.cadence_spm, None);

        let second = &track.points[1];
        assert_eq!(second.time.to_rfc3339(), "2026-10-03T16:12:02+00:00");
        assert_eq!((second.lat, second.lon), (45.0, -45.0));
        assert_eq!(second.heart_rate_bpm, Some(150));
        // 2 × (88 + 64/128) = 177 pasos/min.
        assert_eq!(second.cadence_spm, Some(177.0));
    }

    #[test]
    fn corrupted_fit_is_a_decode_error() {
        let mut fit = FitBuilder::default();
        fit.define(0, 20, &[(253, 4, 0x86)])
            .message(0, &fit_time(0));
        let mut bytes = fit.finish();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        assert!(matches!(read(&bytes), Err(FitError::Decode(_))));
    }
}
