//! Alineación del FIT sintético de Baltanás con las picadas del .spl anonimizado (y, si existe,
//! de un par real de `fixtures/private/`). El método está en `docs/alineacion.md`.

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use tramos_core::alignment::{
    Alignment, AlignmentError, AlignmentOptions, PunchUsage, WarningKind, align,
};
use tramos_core::importers::{fit, spl};
use tramos_core::model::{Event, FINISH_CODE, RaceResult, RaceStatus, START_CODE, Track};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn synthetic_track() -> Track {
    let data = std::fs::read(fixtures().join("fit/baltanas-sintetico.fit")).unwrap();
    fit::read(&data).unwrap()
}

fn spl_bytes() -> Vec<u8> {
    std::fs::read(fixtures().join("spl/baltanas-anon.spl")).unwrap()
}

fn truth() -> Value {
    let text =
        std::fs::read_to_string(fixtures().join("fit/baltanas-sintetico.truth.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// El corredor del FIT sintético: M-SEN, tarjeta 143 (no se usa `Runner.id`, que se repite).
fn synthetic_runner(event: &Event) -> RaceResult {
    event
        .classes
        .iter()
        .find(|c| c.name == "M-SEN")
        .unwrap()
        .results
        .iter()
        .find(|r| r.runner.si_card == Some(143))
        .unwrap()
        .clone()
}

fn load() -> (Track, RaceResult) {
    let event = spl::read(&spl_bytes()).unwrap();
    (synthetic_track(), synthetic_runner(&event))
}

/// Track con todos los instantes desplazados: un reloj que va `seconds` adelantado.
fn shifted(track: &Track, delta: TimeDelta) -> Track {
    let mut t = track.clone();
    for p in &mut t.points {
        p.time += delta;
    }
    t
}

fn kinds(alignment: &Alignment) -> Vec<&WarningKind> {
    alignment.warnings.iter().map(|w| &w.kind).collect()
}

fn instant(text: &str) -> DateTime<Utc> {
    text.parse().unwrap()
}

#[test]
fn estimates_known_shift_within_two_seconds() {
    let (track, result) = load();
    let options = AlignmentOptions::default();
    for shift in [-7, 0, 7, 45] {
        let a = align(
            &shifted(&track, TimeDelta::seconds(shift)),
            &result,
            &options,
        )
        .unwrap();
        let error = a.offset_s - shift as f64;
        assert!(
            error.abs() <= 2.0,
            "desplazado {shift} s: estimado {}",
            a.offset_s
        );
        assert!(a.offset_estimated);
        assert!(
            a.confidence >= 0.8,
            "desplazado {shift} s: confianza {}",
            a.confidence
        );
        // 20 balizas + meta; la salida no cuenta.
        assert_eq!(a.quality.controls_used, 21);
        assert_eq!(a.punches[0].usage, PunchUsage::Start);
        assert!(a.punches[1..].iter().all(|p| p.usage == PunchUsage::Used));
        if shift.abs() <= 7 {
            assert!(a.warnings.is_empty(), "{:?}", a.warnings);
        } else {
            assert_eq!(
                kinds(&a),
                [&WarningKind::LargeOffset {
                    offset_s: a.offset_s
                }]
            );
        }
    }
}

/// Track más parecido a uno real: ruido de GPS de unos metros en cada punto y sin cadencia ni
/// distancia. El desfase debe seguir saliendo a ±2 s.
#[test]
fn tolerates_gps_noise_without_cadence() {
    let (track, result) = load();
    let mut noisy = shifted(&track, TimeDelta::seconds(-7));
    // Generador congruencial determinista: ruido uniforme de ±4 m (≈ ±3,6e-5° de latitud).
    let mut state: u64 = 20_261_003;
    let mut uniform = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    };
    let metre = 1.0 / 111_195.0;
    for p in &mut noisy.points {
        p.lat += 4.0 * metre * uniform();
        p.lon += 4.0 * metre * uniform() / p.lat.to_radians().cos();
        p.cadence_spm = None;
        p.distance_m = None;
    }
    let a = align(&noisy, &result, &AlignmentOptions::default()).unwrap();
    assert!((a.offset_s + 7.0).abs() <= 2.0, "{}", a.offset_s);
    assert!(a.confidence >= 0.5, "{} {:?}", a.confidence, a.quality);
}

#[test]
fn punches_land_on_their_truth_samples() {
    let (track, result) = load();
    let truth = truth();
    let a = align(&track, &result, &AlignmentOptions::default()).unwrap();
    let controls = truth["controls"].as_array().unwrap();
    assert_eq!(a.punches.len(), controls.len());
    for (punch, control) in a.punches.iter().zip(controls) {
        assert_eq!(u64::from(punch.code), control["code"].as_u64().unwrap());
        assert_eq!(
            punch.punch_time,
            Some(instant(control["punch_utc"].as_str().unwrap()))
        );
        let location = punch.location.unwrap();
        let k = control["sample_index"].as_u64().unwrap() as usize;
        // El desfase estimado está a menos de 1 s: la picada cae en su muestra o en la anterior.
        assert!(
            location.index == k || location.index + 1 == k,
            "baliza {}: índice {} frente a {k}",
            punch.code,
            location.index
        );
        assert!(!location.in_gap);
        // `track_time` = picada + desfase, y `location` lo sitúa entre dos puntos.
        let track_time = punch.track_time.unwrap();
        let p = &track.points[location.index];
        let gap = (track_time - p.time).num_milliseconds() as f64 / 1000.0;
        assert!((gap - location.fraction).abs() < 1e-3);
    }
    // Todas (salvo la salida) apoyan el desfase global salvo, como mucho, una.
    let supporting = a
        .punches
        .iter()
        .filter_map(|p| p.local_offset_s)
        .filter(|local| (local - a.offset_s).abs() <= 3.0)
        .count();
    assert!(supporting >= 20, "{supporting}");
}

#[test]
fn coverage_matches_synthetic_track() {
    let (track, result) = load();
    let a = align(&track, &result, &AlignmentOptions::default()).unwrap();
    let c = &a.coverage;
    assert_eq!(c.track_start, instant("2026-10-03T16:12:00Z"));
    assert_eq!(c.track_end, instant("2026-10-03T16:39:40Z"));
    assert_eq!(c.missing_start_s, 0.0);
    assert_eq!(c.missing_end_s, 0.0);
    assert!(c.gaps.is_empty());
    let start_gap = (c.race_start - instant("2026-10-03T16:13:00Z")).num_milliseconds();
    assert_eq!(
        start_gap as f64 / 1000.0,
        (a.offset_s * 1000.0).round() / 1000.0
    );
}

#[test]
fn warns_when_track_does_not_cover_the_race() {
    let (track, result) = load();
    let options = AlignmentOptions::default();

    // Sin los 5 últimos minutos: la meta (muestra 1600) queda 300 s después del último punto.
    let mut short_end = track.clone();
    short_end.points.truncate(1300);
    let a = align(&short_end, &result, &options).unwrap();
    assert!((a.offset_s).abs() <= 2.0, "{}", a.offset_s);
    assert!((a.coverage.missing_end_s - (301.0 + a.offset_s)).abs() < 1e-6);
    assert!(
        kinds(&a)
            .iter()
            .any(|k| matches!(k, WarningKind::TrackEndsEarly { missing_s } if *missing_s > 290.0))
    );
    assert!(a.warnings.iter().any(|w| w.message.contains("acaba")));
    // Las picadas posteriores al final del track no tienen posición ni señal.
    let last = a.punches.last().unwrap();
    assert_eq!(last.location, None);
    assert_eq!(last.usage, PunchUsage::NoSignal);

    // Empieza 3 minutos tarde (120 s después de la salida, que es la muestra 60).
    let mut late = track.clone();
    late.points.drain(..180);
    let a = align(&late, &result, &options).unwrap();
    assert!((a.offset_s).abs() <= 2.0, "{}", a.offset_s);
    assert!(
        kinds(&a)
            .iter()
            .any(|k| matches!(k, WarningKind::TrackStartsLate { missing_s } if *missing_s > 110.0))
    );

    // Hueco de 40 s en mitad de la carrera, que se traga la picada de la 46 (muestra 927).
    let mut gappy = track.clone();
    gappy.points.drain(900..940);
    let a = align(&gappy, &result, &options).unwrap();
    assert!((a.offset_s).abs() <= 2.0, "{}", a.offset_s);
    assert_eq!(a.coverage.gaps.len(), 1);
    assert_eq!(a.coverage.gaps[0].duration_s, 41.0);
    assert!(kinds(&a).contains(&&WarningKind::TrackGaps {
        count: 1,
        longest_s: 41.0,
        max_gap_s: 10.0,
    }));
    let in_gap = a.punches.iter().find(|p| p.code == 46).unwrap();
    assert_eq!(in_gap.usage, PunchUsage::NoSignal);
    assert!(in_gap.location.unwrap().in_gap);
}

fn err_text(track: &Track, result: &RaceResult, hours: i64) -> String {
    align(
        &shifted(track, TimeDelta::hours(hours)),
        result,
        &AlignmentOptions::default(),
    )
    .unwrap_err()
    .to_string()
}

#[test]
fn track_from_another_time_is_an_error() {
    let (track, result) = load();
    // Una hora de diferencia: lo que pasaría con la zona horaria mal convertida.
    let err = align(
        &shifted(&track, TimeDelta::hours(1)),
        &result,
        &AlignmentOptions::default(),
    )
    .unwrap_err();
    assert!(
        matches!(err, AlignmentError::TrackOutsideRace { .. }),
        "{err}"
    );
    assert!(err.to_string().contains("no se solapa"));
    assert!(
        err.to_string().contains("con +1 h sí se solaparía"),
        "{err}"
    );

    // Con ±1 h y ±2 h el error sugiere el desplazamiento (convenio de `offset_s`); con 5 h, no.
    for (hours, want) in [
        (1, Some(3600)),
        (-1, Some(-3600)),
        (2, Some(7200)),
        (-2, Some(-7200)),
        (5, None),
        (-5, None),
    ] {
        let err = align(
            &shifted(&track, TimeDelta::hours(hours)),
            &result,
            &AlignmentOptions::default(),
        )
        .unwrap_err();
        match err {
            AlignmentError::TrackOutsideRace {
                suggested_shift_s, ..
            } => assert_eq!(suggested_shift_s, want, "{hours} h"),
            other => panic!("{hours} h: {other}"),
        }
        if want.is_none() {
            assert!(err_text(&track, &result, hours).contains("¿es el FIT de otra carrera"));
        }
    }

    let empty = Track::default();
    assert_eq!(
        align(&empty, &result, &AlignmentOptions::default()).unwrap_err(),
        AlignmentError::EmptyTrack
    );

    let mut no_times = result.clone();
    for p in &mut no_times.punches {
        p.time = None;
    }
    assert_eq!(
        align(&track, &no_times, &AlignmentOptions::default()).unwrap_err(),
        AlignmentError::NoPunchTimes
    );
}

#[test]
fn punches_without_time_are_ignored() {
    let (track, result) = load();
    let mut partial = result.clone();
    // Sin hora en 5 balizas intermedias.
    for i in [2, 5, 9, 13, 17] {
        partial.punches[i].time = None;
    }
    let shifted_track = shifted(&track, TimeDelta::seconds(7));
    let a = align(&shifted_track, &partial, &AlignmentOptions::default()).unwrap();
    assert!((a.offset_s - 7.0).abs() <= 2.0, "{}", a.offset_s);
    assert_eq!(a.quality.controls_used, 16);
    assert_eq!(kinds(&a), [&WarningKind::PunchesWithoutTime { count: 5 }]);
    for i in [2, 5, 9, 13, 17] {
        let p = &a.punches[i];
        assert_eq!(p.usage, PunchUsage::NoTime);
        assert_eq!(
            (p.track_time, p.location, p.local_offset_s),
            (None, None, None)
        );
    }

    // Sin hora de salida ni de meta: la ventana va de la primera a la última picada con hora.
    let mut open = result.clone();
    open.punches[0].time = None;
    open.punches.last_mut().unwrap().time = None;
    let a = align(&shifted_track, &open, &AlignmentOptions::default()).unwrap();
    assert!((a.offset_s - 7.0).abs() <= 2.0, "{}", a.offset_s);
    assert_eq!(
        kinds(&a),
        [
            &WarningKind::StartWithoutTime,
            &WarningKind::FinishWithoutTime
        ]
    );
    let first_control = result.punches[1].time.unwrap();
    let start_gap = (a.coverage.race_start - first_control).num_milliseconds() as f64 / 1000.0;
    assert!((start_gap - a.offset_s).abs() < 1e-3);
}

#[test]
fn few_usable_punches() {
    let (track, result) = load();
    let options = AlignmentOptions::default();
    let keep = |codes: &[u16]| {
        let mut r = result.clone();
        r.punches.retain(|p| codes.contains(&p.code));
        r
    };

    // Salida, 2 balizas y meta: 3 útiles, el mínimo; se estima pero se avisa.
    let a = align(&track, &keep(&[START_CODE, 49, 42, FINISH_CODE]), &options).unwrap();
    assert!(a.offset_estimated);
    assert_eq!(a.quality.controls_used, 3);
    assert!(kinds(&a).contains(&&WarningKind::FewControls { usable: 3 }));

    // Salida, 1 baliza y meta: no se estima; desfase 0 y confianza 0.
    let a = align(&track, &keep(&[START_CODE, 49, FINISH_CODE]), &options).unwrap();
    assert!(!a.offset_estimated);
    assert_eq!((a.offset_s, a.confidence), (0.0, 0.0));
    assert!(kinds(&a).contains(&&WarningKind::OffsetNotEstimated {
        usable: 2,
        required: 3,
    }));
    assert!(
        a.punches[1..]
            .iter()
            .all(|p| p.usage == PunchUsage::NoSignal)
    );
    // Aun así, cada picada se sitúa en el track con desfase 0.
    assert!(a.punches.iter().all(|p| p.location.is_some()));
}

#[test]
fn punches_of_another_runner_give_low_confidence() {
    // Picadas de otros corredores (otros recorridos y el resto de M-SEN), desplazadas para que
    // salgan a la vez que el del FIT: no deberían cuadrar con el track.
    let event = spl::read(&spl_bytes()).unwrap();
    let track = synthetic_track();
    let options = AlignmentOptions::default();
    let start = synthetic_runner(&event).punches[0].time.unwrap();
    let mut tried = 0;
    for class in &event.classes {
        for other in class
            .results
            .iter()
            .filter(|r| r.status == RaceStatus::Ok && r.runner.si_card != Some(143))
            .take(2)
        {
            let Some(other_start) = other.punches[0].time else {
                continue;
            };
            let mut moved = other.clone();
            for p in &mut moved.punches {
                p.time = p.time.map(|t| start + (t - other_start));
            }
            let a = align(&track, &moved, &options).unwrap();
            assert!(
                a.confidence < options.low_confidence,
                "{} tarjeta {:?}: confianza {}",
                class.name,
                other.runner.si_card,
                a.confidence
            );
            assert!(
                kinds(&a)
                    .iter()
                    .any(|k| matches!(k, WarningKind::LowConfidence { .. })),
                "{:?}",
                a.warnings
            );
            tried += 1;
        }
    }
    assert!(tried >= 30, "{tried}");
}

/// Cambia la fecha (etiqueta `0x19`, días OLE) de la cabecera del .spl.
fn with_event_date(spl: &[u8], from_ole: f64, to_ole: f64) -> Vec<u8> {
    let mut pattern = vec![0x19];
    pattern.extend(from_ole.to_le_bytes());
    let found: Vec<usize> = spl
        .windows(pattern.len())
        .enumerate()
        .filter(|(_, w)| *w == pattern.as_slice())
        .map(|(i, _)| i)
        .collect();
    assert_eq!(found.len(), 1, "fecha {from_ole} en {found:?}");
    let mut out = spl.to_vec();
    out[found[0] + 1..found[0] + 9].copy_from_slice(&to_ole.to_le_bytes());
    out
}

/// La misma carrera, con las mismas horas locales en el .spl, en horario de verano (3 de
/// octubre, UTC+2) y de invierno (12 de diciembre, UTC+1). El reloj graba en UTC y va 7 s
/// adelantado en los dos casos: el desfase estimado debe ser el mismo.
#[test]
fn summer_and_winter_races_align_the_same() {
    let summer_spl = spl_bytes();
    // 46298 = 2026-10-03; 46368 = 2026-12-12 (70 días después).
    let winter_spl = with_event_date(&summer_spl, 46298.0, 46368.0);
    let summer = synthetic_runner(&spl::read(&summer_spl).unwrap());
    let winter = synthetic_runner(&spl::read(&winter_spl).unwrap());

    // Salida a las 18:13:00 locales: 16:13:00Z en verano y 17:13:00Z en invierno.
    assert_eq!(
        summer.punches[0].time,
        Some(instant("2026-10-03T16:13:00Z"))
    );
    assert_eq!(
        winter.punches[0].time,
        Some(instant("2026-12-12T17:13:00Z"))
    );
    // Las picadas del .spl son las mismas horas locales.
    let local = |r: &RaceResult| -> Vec<String> {
        r.punches
            .iter()
            .map(|p| {
                p.time
                    .unwrap()
                    .with_timezone(&spl::RACE_TIME_ZONE)
                    .format("%H:%M:%S")
                    .to_string()
            })
            .collect()
    };
    assert_eq!(local(&summer), local(&winter));

    // El track sintético es de la carrera de verano; el de invierno es el mismo recorrido 70
    // días y 1 h (el cambio de horario) después, en UTC.
    let track = synthetic_track();
    let clock_ahead = TimeDelta::seconds(7);
    let summer_track = shifted(&track, clock_ahead);
    let winter_track = shifted(
        &track,
        TimeDelta::days(70) + TimeDelta::hours(1) + clock_ahead,
    );

    let options = AlignmentOptions::default();
    let a_summer = align(&summer_track, &summer, &options).unwrap();
    let a_winter = align(&winter_track, &winter, &options).unwrap();
    assert!(
        (a_summer.offset_s - 7.0).abs() <= 2.0,
        "{}",
        a_summer.offset_s
    );
    assert!(
        (a_winter.offset_s - 7.0).abs() <= 2.0,
        "{}",
        a_winter.offset_s
    );
    assert!((a_summer.offset_s - a_winter.offset_s).abs() < 1e-9);
    assert_eq!(a_summer.confidence, a_winter.confidence);
    assert!(a_winter.warnings.is_empty(), "{:?}", a_winter.warnings);

    // Si el track de invierno se hubiera convertido con el desfase de verano (UTC+2), estaría
    // una hora antes de la carrera: no se solapa.
    let wrong = shifted(&track, TimeDelta::days(70) + clock_ahead);
    assert!(matches!(
        align(&wrong, &winter, &options),
        Err(AlignmentError::TrackOutsideRace { .. })
    ));
}

#[test]
fn alignment_round_trips_through_json() {
    let (track, mut result) = load();
    result.punches[3].time = None;
    let a = align(
        &shifted(&track, TimeDelta::seconds(40)),
        &result,
        &AlignmentOptions::default(),
    )
    .unwrap();
    let value = serde_json::to_value(&a).unwrap();
    assert_eq!(value["punches"][0]["usage"], json!("start"));
    assert_eq!(value["punches"][3]["usage"], json!("no_time"));
    assert_eq!(value["punches"][3]["track_time"], json!(null));
    assert_eq!(value["punches"][1]["usage"], json!("used"));
    // Avisos aplanados: `kind`, sus datos y `message`.
    let warnings = value["warnings"].as_array().unwrap();
    assert_eq!(warnings[0]["kind"], json!("punches_without_time"));
    assert_eq!(warnings[0]["count"], json!(1));
    assert_eq!(
        warnings[0]["message"],
        json!("1 picada(s) sin hora: no se usan para alinear.")
    );
    assert_eq!(warnings[1]["kind"], json!("large_offset"));
    assert!(
        warnings[1]["message"]
            .as_str()
            .unwrap()
            .starts_with("Desfase de +40,")
    );

    let back: Alignment = serde_json::from_value(value).unwrap();
    assert_eq!(back, a);

    // Las opciones también: un JSON parcial completa con los valores por defecto.
    let options: AlignmentOptions = serde_json::from_value(json!({ "max_offset_s": 30 })).unwrap();
    assert_eq!(options.max_offset_s, 30);
    assert_eq!(
        options.turn_window_s,
        AlignmentOptions::default().turn_window_s
    );
}

#[test]
fn invalid_options_are_rejected() {
    let (track, result) = load();
    let bad = [
        AlignmentOptions {
            turn_window_s: 0,
            ..AlignmentOptions::default()
        },
        AlignmentOptions {
            max_offset_s: 3600,
            ..AlignmentOptions::default()
        },
        AlignmentOptions {
            max_gap_s: f64::NAN,
            ..AlignmentOptions::default()
        },
        AlignmentOptions {
            low_confidence: 2.0,
            ..AlignmentOptions::default()
        },
    ];
    for options in bad {
        assert!(matches!(
            align(&track, &result, &options),
            Err(AlignmentError::InvalidOptions(_))
        ));
    }
}

/// Par real de `fixtures/private/` (fuera del repositorio): `soria-intermedia.fit` y
/// `soria-intermedia.spl`. La categoría (nombre, nombre corto o id) y la posición del corredor
/// en sus resultados se pasan por variables de entorno; si falta algo, el test se salta. Solo
/// imprime datos genéricos: nada de nombres ni coordenadas.
///
/// ```bash
/// TRAMOS_PRIVATE_CLASS=<categoría> TRAMOS_PRIVATE_RESULT_INDEX=<n> \
///     cargo test -p tramos-core --test alignment private_alignment -- --nocapture
/// ```
#[test]
fn private_alignment() {
    let dir = fixtures().join("private");
    let (fit_path, spl_path) = (
        dir.join("soria-intermedia.fit"),
        dir.join("soria-intermedia.spl"),
    );
    if !fit_path.exists() || !spl_path.exists() {
        eprintln!(
            "sin soria-intermedia.fit/.spl en {}: test saltado",
            dir.display()
        );
        return;
    }
    let (Ok(class_key), Ok(index)) = (
        std::env::var("TRAMOS_PRIVATE_CLASS"),
        std::env::var("TRAMOS_PRIVATE_RESULT_INDEX"),
    ) else {
        eprintln!("faltan TRAMOS_PRIVATE_CLASS o TRAMOS_PRIVATE_RESULT_INDEX: test saltado");
        return;
    };
    let index: usize = index.trim().parse().unwrap();

    let track = fit::read(&std::fs::read(&fit_path).unwrap()).unwrap();
    let event = spl::read(&std::fs::read(&spl_path).unwrap()).unwrap();
    let class = event
        .classes
        .iter()
        .find(|c| {
            c.name == class_key
                || c.short_name.as_deref() == Some(class_key.as_str())
                || c.id.to_string() == class_key
        })
        .unwrap_or_else(|| panic!("no hay ninguna categoría {class_key:?}"));
    let result = class
        .results
        .get(index)
        .unwrap_or_else(|| panic!("la categoría tiene {} resultados", class.results.len()));

    let a = align(&track, result, &AlignmentOptions::default()).unwrap();
    eprintln!(
        "desfase {:+.2} s (estimado: {}), confianza {:.2}, picadas usadas {}, apoyo {:.2}, \
         separación {:.2}, alternativa {:?}",
        a.offset_s,
        a.offset_estimated,
        a.confidence,
        a.quality.controls_used,
        a.quality.support,
        a.quality.margin,
        a.quality.runner_up_offset_s,
    );
    eprintln!(
        "cobertura: faltan {:.0} s al principio y {:.0} s al final; {} huecos; el track \
         empieza {:.0} s antes de la salida y acaba {:.0} s después de la meta",
        a.coverage.missing_start_s,
        a.coverage.missing_end_s,
        a.coverage.gaps.len(),
        (a.coverage.race_start - a.coverage.track_start).num_milliseconds() as f64 / 1000.0,
        (a.coverage.track_end - a.coverage.race_finish).num_milliseconds() as f64 / 1000.0,
    );
    for w in &a.warnings {
        eprintln!("aviso: {}", w.message);
    }
    for (i, p) in a.punches.iter().enumerate() {
        // Distancia (s) entre el instante alineado de la picada y el máximo local de la señal.
        let residual = p.local_offset_s.map(|local| local - a.offset_s);
        eprintln!(
            "  {i:>2} baliza {:>5}: {:?}, residuo {}",
            p.code,
            p.usage,
            residual.map_or("-".to_string(), |r| format!("{r:+.0} s")),
        );
    }
    assert!(a.offset_s.abs() <= 60.0);
}
