//! Desfase entre el reloj y el cronometraje de una carrera (#68).
//!
//! La alineación (`docs/alineacion.md`) lo estima sola, pero puede fallar o salir con confianza
//! baja. Aquí se reúne lo que la vista de carrera enseña para corregirlo (el desfase calculado
//! con su confianza, el fijado a mano y el desplazamiento de horas enteras que sugiere la
//! alineación cuando el track no se solapa) y se guarda el desfase manual, escrito a mano o
//! sacado de marcar en el mapa dónde se picó una baliza (#186). Quien lo usa para
//! situar las balizas y cortar los tramos es [`crate::race_map::aligned_legs`], el punto común
//! del mapa, P2 y el histórico.

use serde::Serialize;
use thiserror::Error;
use tramos_core::alignment::{
    AlignmentError, AlignmentOptions, MAX_FIXED_OFFSET_S, MarkError, align, offset_from_mark,
};
use tramos_core::model::{ControlCode, FINISH_CODE, RaceResult, Track};
use tramos_store::{ResultId, Store, StoreError};

use crate::race_map::alignment;

/// Errores al consultar o fijar el desfase. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum OffsetError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("el resultado {0} no está en su carrera")]
    NoSuchResult(i64),
    #[error("Ese desfase no vale: {0}")]
    DoesNotFit(AlignmentError),
    #[error("la baliza {0} no tiene hora de picada: elige otra")]
    NoPunchTime(usize),
    #[error("No se puede sincronizar con ese punto: {0}")]
    Mark(#[from] MarkError),
}

/// Baliza con la que se puede sincronizar el reloj marcándola en el mapa (#186): las que tienen
/// hora de picada, sin la salida (en el .spl suele ser la hora asignada, no una marca física).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SyncControl {
    /// Posición en las picadas, como en el mapa: 1 = primera baliza, …, la última = meta.
    pub position: usize,
    pub code: ControlCode,
    pub is_finish: bool,
}

/// El desfase de una carrera con track, para la vista de carrera.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OffsetView {
    /// Desfase que calcula la alineación automática (s, `instante en el track = picada +
    /// desfase`). `None` si no se puede alinear (`automatic_error`).
    pub automatic_offset_s: Option<f64>,
    /// `false` si no había picadas útiles suficientes y el automático es 0 por defecto.
    pub automatic_estimated: bool,
    /// Confianza del automático, de 0 a 1.
    pub confidence: Option<f64>,
    /// La confianza está por debajo del umbral de aviso de la alineación.
    pub low_confidence: bool,
    /// Avisos de la alineación automática, en español.
    pub automatic_warnings: Vec<String>,
    /// Por qué no se puede alinear automáticamente, en español.
    pub automatic_error: Option<String>,
    /// Desplazamiento de horas enteras (±1 o ±2 h) con el que el track sí se solaparía, si el
    /// automático falla por eso (hora mal convertida).
    pub suggested_shift_s: Option<i64>,
    /// Desfase que queda al aplicar `suggested_shift_s`: el desplazamiento más el desfase fino
    /// que estima la alineación con él (o el desplazamiento solo, si no se puede estimar).
    pub suggested_offset_s: Option<f64>,
    /// Desfase fijado a mano; `None` = automático.
    pub manual_offset_s: Option<f64>,
    /// Mayor desfase manual que se acepta, en valor absoluto (s).
    pub max_manual_offset_s: f64,
    /// Balizas con las que se puede sincronizar en el mapa, en orden.
    pub sync_controls: Vec<SyncControl>,
}

/// Desfase de la carrera del resultado `result_id`; `None` si no tiene track.
pub fn race_offset(store: &Store, result_id: i64) -> Result<Option<OffsetView>, OffsetError> {
    let result = ResultId(result_id);
    let Some(track) = store.load_track(result)? else {
        return Ok(None);
    };
    let race_result = race_result(store, result_id)?;
    Ok(Some(view(
        &track,
        &race_result,
        store.manual_offset(result)?,
    )))
}

/// Fija el desfase de la carrera del resultado `result_id` (`None` vuelve al automático) y
/// devuelve cómo queda. Antes comprueba que con él la carrera se puede situar en el track; si
/// no, no guarda nada. Sin track: `TrackNotFound`.
pub fn set_race_offset(
    store: &mut Store,
    result_id: i64,
    offset_s: Option<f64>,
) -> Result<OffsetView, OffsetError> {
    let result = ResultId(result_id);
    let race_result = race_result(store, result_id)?;
    let track = store
        .load_track(result)?
        .ok_or(StoreError::TrackNotFound(result_id))?;
    if let Some(offset_s) = offset_s {
        alignment(&track, &race_result, Some(offset_s)).map_err(OffsetError::DoesNotFit)?;
    }
    store.set_manual_offset(result, offset_s)?;
    Ok(view(&track, &race_result, offset_s))
}

/// Fija el desfase con el punto (`lon`, `lat`) que el usuario ha marcado en el mapa como el sitio
/// donde picó la baliza `position` (#186, `tramos_core::alignment::offset_from_mark`). Si el
/// track pasa varias veces por ahí, gana la pasada más cercana al desfase en uso (el fijado, el
/// automático o el sugerido, por ese orden). Se guarda como [`set_race_offset`], con la misma
/// comprobación.
pub fn sync_race_offset(
    store: &mut Store,
    result_id: i64,
    position: usize,
    lon: f64,
    lat: f64,
) -> Result<OffsetView, OffsetError> {
    let result = ResultId(result_id);
    let race_result = race_result(store, result_id)?;
    let track = store
        .load_track(result)?
        .ok_or(StoreError::TrackNotFound(result_id))?;
    let punch_time = race_result
        .punches
        .get(position)
        .filter(|_| position > 0)
        .and_then(|p| p.time)
        .ok_or(OffsetError::NoPunchTime(position))?;
    let current = view(&track, &race_result, store.manual_offset(result)?);
    let near_offset_s = current
        .manual_offset_s
        .or(current.automatic_offset_s)
        .or(current.suggested_offset_s)
        .unwrap_or(0.0);
    let marked = offset_from_mark(
        &track,
        punch_time,
        lon,
        lat,
        near_offset_s,
        &AlignmentOptions::default(),
    )?;
    set_race_offset(store, result_id, Some(marked.offset_s))
}

fn race_result(store: &Store, result_id: i64) -> Result<RaceResult, OffsetError> {
    let (event_id, at) = store.result_ref(ResultId(result_id))?;
    let event = store.load_event(event_id)?;
    at.get(&event)
        .cloned()
        .ok_or(OffsetError::NoSuchResult(result_id))
}

fn view(track: &Track, result: &RaceResult, manual_offset_s: Option<f64>) -> OffsetView {
    let options = AlignmentOptions::default();
    let mut view = OffsetView {
        automatic_offset_s: None,
        automatic_estimated: false,
        confidence: None,
        low_confidence: false,
        automatic_warnings: Vec::new(),
        automatic_error: None,
        suggested_shift_s: None,
        suggested_offset_s: None,
        manual_offset_s,
        max_manual_offset_s: MAX_FIXED_OFFSET_S,
        sync_controls: result
            .punches
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, p)| p.time.is_some())
            .map(|(position, p)| SyncControl {
                position,
                code: p.code,
                is_finish: p.code == FINISH_CODE,
            })
            .collect(),
    };
    match align(track, result, &options) {
        Ok(a) => {
            view.automatic_offset_s = Some(a.offset_s);
            view.automatic_estimated = a.offset_estimated;
            view.confidence = Some(a.confidence);
            view.low_confidence = a.confidence < options.low_confidence;
            view.automatic_warnings = a.warnings.into_iter().map(|w| w.message).collect();
        }
        Err(err) => {
            if let AlignmentError::TrackOutsideRace {
                suggested_shift_s: Some(shift_s),
                ..
            } = err
            {
                view.suggested_shift_s = Some(shift_s);
                view.suggested_offset_s = Some(shifted_offset(track, result, shift_s, &options));
            }
            view.automatic_error = Some(format!("No se puede alinear automáticamente: {err}"));
        }
    }
    view
}

/// Desfase al aplicar el desplazamiento `shift_s`: se desplazan las picadas y se estima el
/// desfase fino que queda. Si no se puede estimar, el desplazamiento solo.
fn shifted_offset(
    track: &Track,
    result: &RaceResult,
    shift_s: i64,
    options: &AlignmentOptions,
) -> f64 {
    let shift = chrono::TimeDelta::seconds(shift_s);
    let mut shifted = result.clone();
    for punch in &mut shifted.punches {
        punch.time = punch.time.map(|t| t + shift);
    }
    let fine_s = match align(track, &shifted, options) {
        Ok(a) if a.offset_estimated => a.offset_s,
        _ => 0.0,
    };
    shift_s as f64 + fine_s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::history_view;
    use crate::import::{ImportRequest, import, preview};
    use crate::race_map::{MapTrack, RaceMap, race_map};
    use crate::races::race_breakdown;
    use crate::settings;
    use tramos_core::history::HistoryFilter;
    use tramos_core::identify::RunnerIdentity;

    fn fixture(path: &str) -> String {
        format!("{}/../../fixtures/{path}", env!("CARGO_MANIFEST_DIR"))
    }

    fn identity() -> RunnerIdentity {
        RunnerIdentity {
            si_card: Some(143),
            full_name: Some("N143 Apellido143".into()),
        }
    }

    /// Importa el .spl anonimizado de Baltanás con el FIT sintético como el corredor de la
    /// tarjeta 143, leyendo las horas en la zona `time_zone`.
    fn imported(time_zone: &str, with_fit: bool) -> (Store, i64) {
        let mut store = Store::open_in_memory().unwrap();
        let mut s = settings::load(&store).unwrap();
        s.time_zone = time_zone.into();
        settings::save(&mut store, &s).unwrap();
        let spl = fixture("spl/baltanas-anon.spl");
        let fit = with_fit.then(|| fixture("fit/baltanas-sintetico.fit"));
        let p = preview(&store, &spl, fit.as_deref(), &identity()).unwrap();
        let outcome = import(
            &mut store,
            &ImportRequest {
                spl_path: spl,
                fit_path: fit,
                result: p.candidates[0].result,
                format: p.suggested_format,
                identity: identity(),
            },
        )
        .unwrap();
        (store, outcome.result_id)
    }

    fn ready(store: &Store, result_id: i64) -> MapTrack {
        match race_map(store, result_id).unwrap() {
            RaceMap::Ready(map) => *map,
            other => panic!("se esperaba un mapa: {other:?}"),
        }
    }

    /// Métricas de los tramos que ven P2 y el histórico: desvío y tiempo parado de cada tramo.
    fn breakdown_legs(store: &Store, result_id: i64) -> Vec<(usize, f64, f64)> {
        race_breakdown(store, result_id)
            .unwrap()
            .unwrap()
            .legs
            .iter()
            .map(|l| (l.index, l.detour_s, l.stopped_s))
            .collect()
    }

    #[test]
    fn the_automatic_offset_is_shown_with_its_confidence() {
        let (store, result_id) = imported("Europe/Madrid", true);
        let v = race_offset(&store, result_id).unwrap().unwrap();
        assert!(v.automatic_offset_s.unwrap().abs() < 2.0, "{v:?}");
        assert!(v.automatic_estimated);
        assert!(v.confidence.unwrap() > 0.8);
        assert!(!v.low_confidence);
        assert!(v.automatic_warnings.is_empty());
        assert_eq!(
            (v.automatic_error, v.suggested_shift_s, v.manual_offset_s),
            (None, None, None)
        );

        let (store, result_id) = imported("Europe/Madrid", false);
        assert_eq!(race_offset(&store, result_id).unwrap(), None);
        let mut store = store;
        assert!(matches!(
            set_race_offset(&mut store, result_id, Some(3.0)),
            Err(OffsetError::Store(StoreError::TrackNotFound(_)))
        ));
    }

    /// Criterio de aceptación de #68: cambiar el desfase mueve las balizas en el mapa y cambia
    /// las métricas de los tramos (P2), y volver al automático lo deja como estaba.
    #[test]
    fn changing_the_offset_moves_the_controls_and_the_metrics() {
        let (mut store, result_id) = imported("Europe/Madrid", true);
        let before = ready(&store, result_id);
        let metrics_before = breakdown_legs(&store, result_id);
        let automatic = race_offset(&store, result_id)
            .unwrap()
            .unwrap()
            .automatic_offset_s
            .unwrap();

        // 20 s más de desfase: cada picada cae 20 s más tarde en el track, unos metros más allá.
        let v = set_race_offset(&mut store, result_id, Some(automatic + 20.0)).unwrap();
        assert_eq!(v.manual_offset_s, Some(automatic + 20.0));
        assert_eq!(
            store.manual_offset(ResultId(result_id)).unwrap(),
            v.manual_offset_s
        );
        let after = ready(&store, result_id);
        assert_eq!(after.controls.len(), before.controls.len());
        let moved = before
            .controls
            .iter()
            .zip(&after.controls)
            .filter(|(b, a)| b.coordinate != a.coordinate)
            .count();
        assert!(moved >= 20, "solo se han movido {moved} balizas");
        let first_leg = |m: &MapTrack| m.legs[0].coordinates.clone();
        assert_ne!(first_leg(&before), first_leg(&after));
        let metrics_after = breakdown_legs(&store, result_id);
        assert_eq!(metrics_after.len(), metrics_before.len());
        assert_ne!(metrics_after, metrics_before);

        // Fijar a mano el mismo desfase que el automático deja las balizas donde estaban.
        set_race_offset(&mut store, result_id, Some(automatic)).unwrap();
        assert_eq!(ready(&store, result_id).controls, before.controls);

        // Volver al automático.
        let v = set_race_offset(&mut store, result_id, None).unwrap();
        assert_eq!(v.manual_offset_s, None);
        assert_eq!(ready(&store, result_id), before);
        assert_eq!(breakdown_legs(&store, result_id), metrics_before);
    }

    /// Marcar una baliza donde la sitúa el mapa con un desfase da ese desfase, y en su sitio
    /// verdadero (el FIT sintético pasa por cada baliza justo a la hora de la picada), el
    /// verdadero; también con la zona horaria equivocada, gracias al desfase sugerido.
    #[test]
    fn marking_a_control_on_the_map_sets_the_offset() {
        let (mut store, result_id) = imported("Europe/Madrid", true);
        let v = race_offset(&store, result_id).unwrap().unwrap();
        let controls = &v.sync_controls;
        assert_eq!(controls.len(), 21);
        assert_eq!(controls[0].position, 1);
        assert!(controls.last().unwrap().is_finish);
        assert!(controls.iter().filter(|c| c.is_finish).count() == 1);

        // Con 25 s de desfase, la baliza 7 cae en otro punto; marcar ese punto da 25 s.
        set_race_offset(&mut store, result_id, Some(25.0)).unwrap();
        let shifted = ready(&store, result_id);
        let seventh = shifted.controls.iter().find(|c| c.position == 7).unwrap();
        set_race_offset(&mut store, result_id, None).unwrap();
        let v = sync_race_offset(
            &mut store,
            result_id,
            7,
            seventh.coordinate[0],
            seventh.coordinate[1],
        )
        .unwrap();
        let manual = v.manual_offset_s.unwrap();
        assert!((manual - 25.0).abs() < 0.5, "{manual}");
        assert_eq!(
            store.manual_offset(ResultId(result_id)).unwrap(),
            Some(manual)
        );

        // Zona horaria equivocada: el automático no encaja, pero el punto verdadero de la
        // baliza da el desfase de la zona buena (−1 h y el fino).
        let truth: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(fixture("fit/baltanas-sintetico.truth.json")).unwrap(),
        )
        .unwrap();
        let c = &truth["controls"][7];
        let (mut store, result_id) = imported("Atlantic/Canary", true);
        let v = sync_race_offset(
            &mut store,
            result_id,
            7,
            c["lon"].as_f64().unwrap(),
            c["lat"].as_f64().unwrap(),
        )
        .unwrap();
        let manual = v.manual_offset_s.unwrap();
        assert!((manual + 3600.0).abs() < 1.0, "{manual}");
        assert!(matches!(
            race_map(&store, result_id).unwrap(),
            RaceMap::Ready(_)
        ));
    }

    #[test]
    fn marking_needs_a_punch_with_time_and_a_point_on_the_track() {
        let (mut store, result_id) = imported("Europe/Madrid", true);
        let map = ready(&store, result_id);
        let [lon, lat] = map.controls[3].coordinate;
        // La salida no vale (hora asignada), ni una posición que no existe.
        for position in [0, 99] {
            assert!(matches!(
                sync_race_offset(&mut store, result_id, position, lon, lat),
                Err(OffsetError::NoPunchTime(p)) if p == position
            ));
        }
        let err = sync_race_offset(&mut store, result_id, 3, lon, lat + 0.05).unwrap_err();
        assert!(
            matches!(err, OffsetError::Mark(MarkError::TooFar { .. })),
            "{err}"
        );
        assert!(err.to_string().contains("pulsa sobre la línea"), "{err}");
        assert_eq!(store.manual_offset(ResultId(result_id)).unwrap(), None);
    }

    /// Un desfase con el que la carrera no cae en el track no se guarda.
    #[test]
    fn an_offset_outside_the_track_is_rejected() {
        let (mut store, result_id) = imported("Europe/Madrid", true);
        let err = set_race_offset(&mut store, result_id, Some(7200.0)).unwrap_err();
        assert!(matches!(err, OffsetError::DoesNotFit(_)), "{err}");
        assert!(err.to_string().starts_with("Ese desfase no vale"), "{err}");
        for bad in [f64::NAN, 1e9] {
            assert!(matches!(
                set_race_offset(&mut store, result_id, Some(bad)),
                Err(OffsetError::DoesNotFit(
                    AlignmentError::InvalidOffset { .. }
                ))
            ));
        }
        assert_eq!(store.manual_offset(ResultId(result_id)).unwrap(), None);
        assert!(matches!(
            set_race_offset(&mut store, 9_999, None),
            Err(OffsetError::Store(StoreError::ResultNotFound(9_999)))
        ));
    }

    /// Con la zona horaria equivocada (Canarias va una hora por detrás de la península), el
    /// track se guarda igualmente, el automático no encaja y sugiere −1 h. Aplicar la sugerencia
    /// deja las balizas donde las sitúa la importación con la zona buena.
    #[test]
    fn the_suggested_shift_fixes_a_wrong_time_zone() {
        let (good_store, good_id) = imported("Europe/Madrid", true);
        let good = ready(&good_store, good_id);
        let good_offset = race_offset(&good_store, good_id)
            .unwrap()
            .unwrap()
            .automatic_offset_s
            .unwrap();

        let (mut store, result_id) = imported("Atlantic/Canary", true);
        assert!(matches!(
            race_map(&store, result_id).unwrap(),
            RaceMap::NotAligned { .. }
        ));
        assert!(race_breakdown(&store, result_id).unwrap().is_none());
        let races_with_track = |store: &Store| {
            history_view(store, &HistoryFilter::default())
                .unwrap()
                .by_slope
                .races_with_track
        };
        assert_eq!(races_with_track(&store), 0);
        let v = race_offset(&store, result_id).unwrap().unwrap();
        assert_eq!(v.automatic_offset_s, None);
        assert!(
            v.automatic_error.as_deref().unwrap().contains("1 h"),
            "{v:?}"
        );
        assert_eq!(v.suggested_shift_s, Some(-3600));
        let suggested = v.suggested_offset_s.unwrap();
        assert!(
            (suggested - (-3600.0 + good_offset)).abs() < 0.5,
            "{suggested} frente a {good_offset}"
        );

        set_race_offset(&mut store, result_id, Some(suggested)).unwrap();
        let fixed = ready(&store, result_id);
        assert_eq!(fixed.controls.len(), good.controls.len());
        for (a, b) in fixed.controls.iter().zip(&good.controls) {
            let d = ((a.coordinate[0] - b.coordinate[0]).powi(2)
                + (a.coordinate[1] - b.coordinate[1]).powi(2))
            .sqrt();
            // Menos de 1e-5° (alrededor de 1 m).
            assert!(d < 1e-5, "baliza {}: {d}", a.code);
        }
        // P2 y el histórico (P13) ya tienen los tramos de la carrera.
        assert!(race_breakdown(&store, result_id).unwrap().is_some());
        assert_eq!(races_with_track(&store), 1);
        // La sugerencia sigue a la vista mientras el automático no encaje.
        let v = race_offset(&store, result_id).unwrap().unwrap();
        assert_eq!(
            (v.manual_offset_s, v.suggested_shift_s),
            (Some(suggested), Some(-3600))
        );
    }
}
