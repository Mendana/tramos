//! Ajustes del usuario: umbrales del tiempo perdido, zona horaria de las carreras e identidad.
//!
//! Se guardan en la tabla de ajustes clave-valor de la base (`docs/almacenamiento.md`). Las
//! claves y su efecto están en `docs/app.md`, "Ajustes".

use std::str::FromStr;

use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tramos_core::identify::RunnerIdentity;
use tramos_core::importers::spl::RACE_TIME_ZONE;
use tramos_core::lost_time::LostTimeConfig;
use tramos_store::{Store, StoreError};

/// Umbral de error en segundos.
pub const ERROR_THRESHOLD_S_KEY: &str = "lost_time.error_threshold_s";
/// Umbral de error en % del tiempo esperado.
pub const ERROR_THRESHOLD_PCT_KEY: &str = "lost_time.error_threshold_pct";
/// Zona horaria IANA de las horas del .spl al importar.
pub const TIME_ZONE_KEY: &str = "import.time_zone";
/// Tarjeta SI del usuario, para buscarlo en cada carrera.
pub const SI_CARD_KEY: &str = "self.si_card";
/// Nombre y apellidos del usuario, para lo mismo.
pub const FULL_NAME_KEY: &str = "self.full_name";

/// Errores de los ajustes. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("el umbral en segundos tiene que ser un número mayor o igual que 0")]
    InvalidThresholdS,
    #[error("el umbral en % tiene que ser un número mayor o igual que 0")]
    InvalidThresholdPct,
    #[error("zona horaria desconocida: {0:?} (por ejemplo, Europe/Madrid o Atlantic/Canary)")]
    InvalidTimeZone(String),
    #[error("la tarjeta SI tiene que ser un número mayor que 0")]
    InvalidSiCard,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Todos los ajustes, tal y como los edita la interfaz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Pérdida mínima en segundos para que un tramo sea error.
    pub error_threshold_s: f64,
    /// Pérdida mínima en % del tiempo esperado (10 = 10 %).
    pub error_threshold_pct: f64,
    /// Zona horaria IANA de las horas del .spl.
    pub time_zone: String,
    pub identity: RunnerIdentity,
}

/// Ajustes guardados; los que faltan (o no se entienden) toman el valor por defecto.
pub fn load(store: &Store) -> Result<Settings, StoreError> {
    let defaults = LostTimeConfig::default();
    let number = |key: &str, default: f64| -> Result<f64, StoreError> {
        Ok(store
            .setting(key)?
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0)
            .unwrap_or(default))
    };
    Ok(Settings {
        error_threshold_s: number(ERROR_THRESHOLD_S_KEY, defaults.error_threshold_s)?,
        error_threshold_pct: number(ERROR_THRESHOLD_PCT_KEY, defaults.error_threshold_pct)?,
        time_zone: store
            .setting(TIME_ZONE_KEY)?
            .filter(|v| Tz::from_str(v).is_ok())
            .unwrap_or_else(|| RACE_TIME_ZONE.name().to_string()),
        identity: identity(store)?,
    })
}

/// Valida y guarda todos los ajustes. Si alguno no vale, no se guarda ninguno.
pub fn save(store: &mut Store, settings: &Settings) -> Result<(), SettingsError> {
    let valid = |v: f64| v.is_finite() && v >= 0.0;
    if !valid(settings.error_threshold_s) {
        return Err(SettingsError::InvalidThresholdS);
    }
    if !valid(settings.error_threshold_pct) {
        return Err(SettingsError::InvalidThresholdPct);
    }
    let zone = settings.time_zone.trim();
    if Tz::from_str(zone).is_err() {
        return Err(SettingsError::InvalidTimeZone(zone.to_string()));
    }
    if settings.identity.si_card == Some(0) {
        return Err(SettingsError::InvalidSiCard);
    }
    store.set_setting(
        ERROR_THRESHOLD_S_KEY,
        &settings.error_threshold_s.to_string(),
    )?;
    store.set_setting(
        ERROR_THRESHOLD_PCT_KEY,
        &settings.error_threshold_pct.to_string(),
    )?;
    store.set_setting(TIME_ZONE_KEY, zone)?;
    save_identity(store, &settings.identity, true)?;
    Ok(())
}

/// Configuración del tiempo perdido con los umbrales guardados.
pub fn lost_time_config(store: &Store) -> Result<LostTimeConfig, StoreError> {
    let settings = load(store)?;
    Ok(LostTimeConfig {
        error_threshold_s: settings.error_threshold_s,
        error_threshold_pct: settings.error_threshold_pct,
        ..LostTimeConfig::default()
    })
}

/// Zona horaria guardada para leer el .spl.
pub fn time_zone(store: &Store) -> Result<Tz, StoreError> {
    let name = load(store)?.time_zone;
    Ok(Tz::from_str(&name).unwrap_or(RACE_TIME_ZONE))
}

/// Identidad guardada (tarjeta y nombre).
pub fn identity(store: &Store) -> Result<RunnerIdentity, StoreError> {
    Ok(RunnerIdentity {
        si_card: store.setting(SI_CARD_KEY)?.and_then(|v| v.parse().ok()),
        full_name: store
            .setting(FULL_NAME_KEY)?
            .filter(|v| !v.trim().is_empty()),
    })
}

/// Guarda la identidad. Con `clear_missing`, un campo vacío borra el guardado (pantalla de
/// ajustes); sin él, un campo vacío no toca lo que había (al importar).
pub fn save_identity(
    store: &mut Store,
    identity: &RunnerIdentity,
    clear_missing: bool,
) -> Result<(), StoreError> {
    match identity.si_card {
        Some(card) => store.set_setting(SI_CARD_KEY, &card.to_string())?,
        None if clear_missing => store.set_setting(SI_CARD_KEY, "")?,
        None => {}
    }
    match identity.full_name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => store.set_setting(FULL_NAME_KEY, name)?,
        _ if clear_missing => store.set_setting(FULL_NAME_KEY, "")?,
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings {
            error_threshold_s: 20.0,
            error_threshold_pct: 12.5,
            time_zone: "Atlantic/Canary".into(),
            identity: RunnerIdentity {
                si_card: Some(143),
                full_name: Some("N143 Apellido143".into()),
            },
        }
    }

    #[test]
    fn defaults_without_anything_saved() {
        let store = Store::open_in_memory().unwrap();
        let s = load(&store).unwrap();
        assert_eq!((s.error_threshold_s, s.error_threshold_pct), (15.0, 10.0));
        assert_eq!(s.time_zone, "Europe/Madrid");
        assert_eq!(s.identity, RunnerIdentity::default());
        assert_eq!(lost_time_config(&store).unwrap(), LostTimeConfig::default());
        assert_eq!(time_zone(&store).unwrap(), RACE_TIME_ZONE);
    }

    #[test]
    fn saved_settings_round_trip() {
        let mut store = Store::open_in_memory().unwrap();
        save(&mut store, &settings()).unwrap();
        assert_eq!(load(&store).unwrap(), settings());
        let config = lost_time_config(&store).unwrap();
        assert_eq!(
            (config.error_threshold_s, config.error_threshold_pct),
            (20.0, 12.5)
        );
        assert_eq!(time_zone(&store).unwrap(), chrono_tz::Atlantic::Canary);

        // En la pantalla de ajustes, vaciar la identidad la borra.
        let mut cleared = settings();
        cleared.identity = RunnerIdentity::default();
        save(&mut store, &cleared).unwrap();
        assert_eq!(load(&store).unwrap().identity, RunnerIdentity::default());
    }

    #[test]
    fn importing_without_identity_keeps_the_saved_one() {
        let mut store = Store::open_in_memory().unwrap();
        save(&mut store, &settings()).unwrap();
        save_identity(&mut store, &RunnerIdentity::default(), false).unwrap();
        assert_eq!(identity(&store).unwrap(), settings().identity);
    }

    #[test]
    fn invalid_settings_save_nothing() {
        let mut store = Store::open_in_memory().unwrap();
        let bad = |change: fn(&mut Settings)| {
            let mut s = settings();
            change(&mut s);
            s
        };
        let cases = [
            bad(|s| s.error_threshold_s = -1.0),
            bad(|s| s.error_threshold_pct = f64::NAN),
            bad(|s| s.time_zone = "Madrid".into()),
            bad(|s| s.identity.si_card = Some(0)),
        ];
        for case in &cases {
            assert!(save(&mut store, case).is_err(), "{case:?}");
        }
        assert_eq!(load(&store).unwrap().time_zone, "Europe/Madrid");
        assert_eq!(load(&store).unwrap().error_threshold_s, 15.0);
    }
}
