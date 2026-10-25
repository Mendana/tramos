//! Pestaña Físico de una carrera (#213): el rendimiento físico del resultado y la serie para sus
//! gráficas. Todo lo calcula el núcleo (`tramos_core::physical`, `tramos_core::physical_series`;
//! `docs/rendimiento-fisico.md`); la interfaz solo dibuja lo que recibe.
//!
//! Se alinea y trocea el track guardado como el mapa ([`crate::race_map::aligned_legs`], con el
//! desfase fijado a mano si lo tiene) y las zonas de pulso salen de los ajustes (`docs/app.md`,
//! "Ajustes"): sus límites van al núcleo y sus colores a la interfaz.

use serde::Serialize;
use thiserror::Error;
use tramos_core::metrics::{MetricsError, MetricsOptions};
use tramos_core::physical::{RacePhysical, race_physical};
use tramos_core::physical_series::{RaceSeries, race_series};
use tramos_store::{ResultId, Store, StoreError};

use crate::race_map::aligned_legs;
use crate::settings;
use crate::zones::Zones;

/// Segundos entre un instante de la serie y el siguiente.
pub const SERIES_STEP_S: f64 = 5.0;

/// Errores al calcular el rendimiento físico. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum PhysicalError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Metrics(#[from] MetricsError),
    #[error("el resultado {0} no está en su carrera")]
    NoSuchResult(i64),
}

/// Lo que necesita la interfaz para dibujar la pestaña Físico de un resultado.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PhysicalView {
    /// La carrera se importó sin FIT: no hay rendimiento físico.
    NoTrack,
    /// Hay track, pero no se puede alinear con las picadas (como `RaceMap::NotAligned`).
    /// `message` dice por qué, en español.
    NotAligned { message: String },
    /// Cifras y serie listas para dibujar.
    Ready(Box<PhysicalReady>),
}

/// Rendimiento físico de un resultado con track.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhysicalReady {
    pub physical: RacePhysical,
    /// De la salida a la meta cada [`SERIES_STEP_S`] s.
    pub series: RaceSeries,
    /// Las zonas de pulso de Ajustes, con sus colores; `None` si no hay.
    pub zones: Option<Zones>,
}

/// El rendimiento físico del resultado `result_id`.
pub fn race_physical_view(store: &Store, result_id: i64) -> Result<PhysicalView, PhysicalError> {
    let result = ResultId(result_id);
    let (event_id, at) = store.result_ref(result)?;
    let Some(track) = store.load_track(result)? else {
        return Ok(PhysicalView::NoTrack);
    };
    let event = store.load_event(event_id)?;
    let race_result = at
        .get(&event)
        .ok_or(PhysicalError::NoSuchResult(result_id))?;
    let manual_offset_s = store.manual_offset(result)?;
    let segmentation = match aligned_legs(&track, race_result, manual_offset_s) {
        Ok((_, segmentation)) => segmentation,
        Err(message) => return Ok(PhysicalView::NotAligned { message }),
    };

    let zones = settings::load(store)?.map.heart_rate_zones;
    let options = MetricsOptions::default();
    let physical = race_physical(
        &track,
        &segmentation,
        &options,
        zones.as_ref().map(|z| z.limits.as_slice()),
    )?;
    let series = race_series(&track, &segmentation, &options, SERIES_STEP_S)?;
    Ok(PhysicalView::Ready(Box::new(PhysicalReady {
        physical,
        series,
        zones,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::race_map::tests::imported;
    use tramos_core::physical::SpeedKind;
    use tramos_core::physical_series::SeriesState;

    fn ready(view: PhysicalView) -> PhysicalReady {
        match view {
            PhysicalView::Ready(ready) => *ready,
            other => panic!("se esperaba la pestaña lista: {other:?}"),
        }
    }

    #[test]
    fn a_race_without_fit_has_no_physical_view() {
        let (store, result_id) = imported(false);
        assert_eq!(
            race_physical_view(&store, result_id).unwrap(),
            PhysicalView::NoTrack
        );
        let json = serde_json::to_value(PhysicalView::NoTrack).unwrap();
        assert_eq!(json, serde_json::json!({"status": "no_track"}));
    }

    #[test]
    fn missing_result_is_an_error() {
        let store = Store::open_in_memory().unwrap();
        assert!(matches!(
            race_physical_view(&store, 42),
            Err(PhysicalError::Store(StoreError::ResultNotFound(42)))
        ));
    }

    /// El FIT sintético, sin zonas en Ajustes: cifras y serie del núcleo, sobre los mismos tramos
    /// que el mapa.
    #[test]
    fn the_synthetic_race_has_figures_and_series() {
        let (store, result_id) = imported(true);
        let r = ready(race_physical_view(&store, result_id).unwrap());
        assert_eq!(r.zones, None);
        assert_eq!(r.physical.heart_rate_zones, None);
        assert_eq!(r.physical.speed, SpeedKind::Flat);
        assert!(r.physical.transit.efficiency_m_per_beat.is_some());
        assert_eq!(r.physical.blocks.len(), tramos_core::physical::BLOCKS);

        // Una serie cada 5 s, de la salida a la meta, con los 21 tramos del mapa.
        assert_eq!(r.series.step_s, SERIES_STEP_S);
        assert!((r.series.duration_s - r.physical.duration_s).abs() < 1.0);
        assert_eq!(r.series.legs.len(), 21);
        let expected = (r.series.duration_s / SERIES_STEP_S).floor() as usize + 1;
        assert!(r.series.samples.len() >= expected);
        assert!(
            r.series
                .samples
                .iter()
                .any(|s| s.state == SeriesState::Attack)
        );
        let map = match crate::race_map::race_map(&store, result_id).unwrap() {
            crate::race_map::RaceMap::Ready(map) => map,
            other => panic!("se esperaba un mapa: {other:?}"),
        };
        assert_eq!(map.legs.len(), r.series.legs.len());
    }

    /// Los límites de las zonas de Ajustes llegan al núcleo y sus colores, a la interfaz.
    #[test]
    fn heart_rate_zones_come_from_the_settings() {
        let (mut store, result_id) = imported(true);
        let zones = Zones {
            limits: vec![150.0, 165.0, 180.0],
            colors: vec![
                "#6b7280".into(),
                "#2563eb".into(),
                "#15803d".into(),
                "#b91c1c".into(),
            ],
        };
        store
            .set_setting(
                settings::HEART_RATE_ZONES_KEY,
                &Zones::to_setting(Some(&zones)),
            )
            .unwrap();
        let r = ready(race_physical_view(&store, result_id).unwrap());
        assert_eq!(r.zones.as_ref(), Some(&zones));
        let seconds = r.physical.heart_rate_zones.unwrap().seconds;
        assert_eq!(seconds.len(), 4);
        assert!(seconds.iter().sum::<f64>() > 0.0);
    }

    /// Con un desfase que no encaja, la pestaña lo dice, como el mapa.
    #[test]
    fn an_unaligned_track_says_why() {
        let (mut store, result_id) = imported(true);
        store
            .set_manual_offset(ResultId(result_id), Some(7200.0))
            .unwrap();
        match race_physical_view(&store, result_id).unwrap() {
            PhysicalView::NotAligned { message } => {
                assert!(
                    message.starts_with("No se puede situar la carrera"),
                    "{message}"
                );
            }
            other => panic!("se esperaba NotAligned: {other:?}"),
        }
    }
}
