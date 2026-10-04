//! Conversión entre los tipos del modelo y los valores de las columnas.

use chrono::{DateTime, NaiveDate, Utc};
use tramos_core::model::{RaceStatus, Sex};

use crate::StoreError;

/// Instante → milisegundos desde la época Unix (UTC). Falla si la vuelta no sería exacta:
/// precisión por debajo del milisegundo o segundo intercalar.
pub(crate) fn instant_to_ms(t: DateTime<Utc>) -> Result<i64, StoreError> {
    let nanos = t.timestamp_subsec_nanos();
    if nanos >= 1_000_000_000 || nanos % 1_000_000 != 0 {
        return Err(StoreError::SubMillisecondInstant(t));
    }
    Ok(t.timestamp_millis())
}

pub(crate) fn ms_to_instant(ms: i64) -> Result<DateTime<Utc>, StoreError> {
    DateTime::from_timestamp_millis(ms)
        .ok_or_else(|| StoreError::InvalidData(format!("instante fuera de rango: {ms} ms")))
}

/// Fecha local → `AAAA-MM-DD`.
pub(crate) fn date_to_text(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub(crate) fn text_to_date(text: &str) -> Result<NaiveDate, StoreError> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .map_err(|_| StoreError::InvalidData(format!("fecha inválida: {text:?}")))
}

/// Estado → (`status`, `status_code`). El código solo existe para `unknown`.
pub(crate) fn status_to_sql(status: RaceStatus) -> (&'static str, Option<u8>) {
    match status {
        RaceStatus::Ok => ("ok", None),
        RaceStatus::NotClassified => ("not_classified", None),
        RaceStatus::DidNotStart => ("did_not_start", None),
        RaceStatus::Unknown(code) => ("unknown", Some(code)),
    }
}

pub(crate) fn sql_to_status(status: &str, code: Option<u8>) -> Result<RaceStatus, StoreError> {
    match (status, code) {
        ("ok", None) => Ok(RaceStatus::Ok),
        ("not_classified", None) => Ok(RaceStatus::NotClassified),
        ("did_not_start", None) => Ok(RaceStatus::DidNotStart),
        ("unknown", Some(code)) => Ok(RaceStatus::Unknown(code)),
        _ => Err(StoreError::InvalidData(format!(
            "estado inválido: {status:?} con código {code:?}"
        ))),
    }
}

pub(crate) fn sex_to_sql(sex: Sex) -> &'static str {
    match sex {
        Sex::Male => "male",
        Sex::Female => "female",
    }
}

pub(crate) fn sql_to_sex(text: &str) -> Result<Sex, StoreError> {
    match text {
        "male" => Ok(Sex::Male),
        "female" => Ok(Sex::Female),
        _ => Err(StoreError::InvalidData(format!("sexo inválido: {text:?}"))),
    }
}

/// Índice de una lista del modelo → columna `position`.
pub(crate) fn position(index: usize) -> Result<i64, StoreError> {
    i64::try_from(index).map_err(|_| StoreError::InvalidData(format!("posición {index}")))
}

/// Rechaza NaN, que SQLite guardaría como NULL.
pub(crate) fn real(value: f64, field: &'static str) -> Result<f64, StoreError> {
    if value.is_nan() {
        Err(StoreError::NotANumber(field))
    } else {
        Ok(value)
    }
}

pub(crate) fn opt_real(value: Option<f64>, field: &'static str) -> Result<Option<f64>, StoreError> {
    value.map(|v| real(v, field)).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn instants_are_whole_milliseconds_since_epoch() {
        let t = Utc.with_ymd_and_hms(2026, 10, 3, 9, 0, 0).unwrap();
        assert_eq!(instant_to_ms(t).unwrap(), 1_791_018_000_000);
        assert_eq!(ms_to_instant(1_791_018_000_000).unwrap(), t);

        let with_ms = t + chrono::Duration::milliseconds(250);
        assert_eq!(instant_to_ms(with_ms).unwrap(), 1_791_018_000_250);
        assert_eq!(ms_to_instant(1_791_018_000_250).unwrap(), with_ms);
    }

    #[test]
    fn sub_millisecond_instants_are_rejected() {
        let t =
            Utc.with_ymd_and_hms(2026, 10, 3, 9, 0, 0).unwrap() + chrono::Duration::microseconds(1);
        assert!(matches!(
            instant_to_ms(t),
            Err(StoreError::SubMillisecondInstant(_))
        ));
    }

    #[test]
    fn dates_are_iso_text() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        assert_eq!(date_to_text(date), "2026-01-05");
        assert_eq!(text_to_date("2026-01-05").unwrap(), date);
        assert!(text_to_date("05/01/2026").is_err());
    }

    #[test]
    fn statuses_round_trip() {
        for status in [
            RaceStatus::Ok,
            RaceStatus::NotClassified,
            RaceStatus::DidNotStart,
            RaceStatus::Unknown(6),
        ] {
            let (text, code) = status_to_sql(status);
            assert_eq!(sql_to_status(text, code).unwrap(), status);
        }
        assert_eq!(status_to_sql(RaceStatus::Unknown(6)), ("unknown", Some(6)));
        assert!(sql_to_status("ok", Some(1)).is_err());
        assert!(sql_to_status("unknown", None).is_err());
    }

    #[test]
    fn nan_is_rejected() {
        assert!(matches!(
            real(f64::NAN, "lat"),
            Err(StoreError::NotANumber("lat"))
        ));
        assert_eq!(opt_real(None, "x").unwrap(), None);
        assert_eq!(opt_real(Some(1.5), "x").unwrap(), Some(1.5));
    }
}
