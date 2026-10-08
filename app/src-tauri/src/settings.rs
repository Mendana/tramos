//! Ajustes del usuario: umbrales del tiempo perdido, zona horaria de las carreras, identidad y
//! carpeta compartida.
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
use tramos_core::package::ShareChoice;
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
/// Modo de la app: `runner` (exporta sus carreras) o `coach` (recibe las de los corredores).
pub const MODE_KEY: &str = "sharing.mode";
/// Carpeta compartida (sincronizada con Drive, OneDrive, Dropbox…); vacía = ninguna.
pub const FOLDER_KEY: &str = "sharing.folder";
/// Qué se comparte de una carrera si el corredor no ha elegido nada para ella.
pub const DEFAULT_CHOICE_KEY: &str = "sharing.default_choice";

/// Lo que se comparte por defecto: los tramos y las etiquetas, sin pulso ni GPS.
pub const DEFAULT_SHARE_CHOICE: ShareChoice = ShareChoice::Legs;

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
    #[error("la carpeta compartida no existe o no es una carpeta: {0}")]
    InvalidFolder(String),
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
    pub sharing: SharingSettings,
}

/// Quién usa la app (`docs/paquete.md`, "Carpeta compartida").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppMode {
    /// Un corredor: exporta sus carreras a la carpeta.
    Runner,
    /// La entrenadora: importa los paquetes que dejan los corredores en la carpeta.
    Coach,
}

/// Cómo se comparte con la entrenadora.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharingSettings {
    pub mode: AppMode,
    /// Carpeta compartida; `None` = sin compartir.
    pub folder: Option<String>,
    /// Qué se comparte de una carrera si no se ha elegido nada para ella.
    pub default_choice: ShareChoice,
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
        sharing: SharingSettings {
            mode: match store.setting(MODE_KEY)?.as_deref() {
                Some("coach") => AppMode::Coach,
                _ => AppMode::Runner,
            },
            folder: store.setting(FOLDER_KEY)?.filter(|v| !v.trim().is_empty()),
            default_choice: store
                .setting(DEFAULT_CHOICE_KEY)?
                .and_then(|v| ShareChoice::from_key(&v))
                .unwrap_or(DEFAULT_SHARE_CHOICE),
        },
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
    let folder = settings
        .sharing
        .folder
        .as_deref()
        .map(str::trim)
        .filter(|f| !f.is_empty());
    if let Some(missing) = folder.filter(|f| !std::path::Path::new(f).is_dir()) {
        return Err(SettingsError::InvalidFolder(missing.to_string()));
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
    choose_mode(store, settings.sharing.mode)?;
    store.set_setting(FOLDER_KEY, folder.unwrap_or(""))?;
    store.set_setting(DEFAULT_CHOICE_KEY, settings.sharing.default_choice.key())?;
    Ok(())
}

/// Si ya se ha elegido el modo: está guardado o, en una base de antes del modo, ya hay
/// carreras importadas (entonces es un corredor).
pub fn mode_chosen(store: &Store) -> Result<bool, StoreError> {
    Ok(store.setting(MODE_KEY)?.is_some() || crate::import::stored_self_person(store)?.is_some())
}

/// Guarda el modo sin tocar los demás ajustes.
pub fn choose_mode(store: &mut Store, mode: AppMode) -> Result<(), StoreError> {
    store.set_setting(
        MODE_KEY,
        match mode {
            AppMode::Runner => "runner",
            AppMode::Coach => "coach",
        },
    )
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
            sharing: SharingSettings {
                mode: AppMode::Runner,
                folder: None,
                default_choice: ShareChoice::Legs,
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
        assert_eq!(
            s.sharing,
            SharingSettings {
                mode: AppMode::Runner,
                folder: None,
                default_choice: ShareChoice::Legs
            }
        );
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
            bad(|s| s.sharing.folder = Some("/no/existe/esta/carpeta".into())),
        ];
        for case in &cases {
            assert!(save(&mut store, case).is_err(), "{case:?}");
        }
        assert_eq!(load(&store).unwrap().time_zone, "Europe/Madrid");
        assert_eq!(load(&store).unwrap().error_threshold_s, 15.0);
    }

    #[test]
    fn the_shared_folder_and_mode_round_trip() {
        let mut store = Store::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut s = settings();
        s.sharing = SharingSettings {
            mode: AppMode::Coach,
            folder: Some(dir.path().to_string_lossy().into_owned()),
            default_choice: ShareChoice::Nothing,
        };
        save(&mut store, &s).unwrap();
        assert_eq!(load(&store).unwrap(), s);

        // Una carpeta en blanco es ninguna.
        s.sharing.folder = Some("  ".into());
        save(&mut store, &s).unwrap();
        assert_eq!(load(&store).unwrap().sharing.folder, None);

        // Valores que no se entienden toman el valor por defecto.
        store.set_setting(MODE_KEY, "otro").unwrap();
        store.set_setting(DEFAULT_CHOICE_KEY, "todo").unwrap();
        let sharing = load(&store).unwrap().sharing;
        assert_eq!(
            (sharing.mode, sharing.default_choice),
            (AppMode::Runner, DEFAULT_SHARE_CHOICE)
        );
    }

    #[test]
    fn the_mode_is_chosen_once() {
        let mut store = Store::open_in_memory().unwrap();
        assert!(!mode_chosen(&store).unwrap());
        choose_mode(&mut store, AppMode::Coach).unwrap();
        assert!(mode_chosen(&store).unwrap());
        assert_eq!(load(&store).unwrap().sharing.mode, AppMode::Coach);
        // Elegirlo no toca los demás ajustes.
        assert_eq!(load(&store).unwrap().time_zone, "Europe/Madrid");
    }

    #[test]
    fn a_runner_from_before_the_mode_has_it_chosen() {
        let (store, _) = crate::race_map::tests::imported(false);
        assert!(mode_chosen(&store).unwrap());
        assert_eq!(load(&store).unwrap().sharing.mode, AppMode::Runner);
    }
}
