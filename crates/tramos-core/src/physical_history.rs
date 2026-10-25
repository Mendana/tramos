//! Rendimiento físico en el histórico (#214): cómo evoluciona el físico de carrera en carrera y
//! qué hace el corredor de media en cada pendiente y en cada décima de la carrera. Las
//! definiciones están en `docs/rendimiento-fisico.md`, "Histórico", y en `docs/historico.md`.
//!
//! Parte de [`RacePhysical`] de cada carrera con FIT (`crate::physical::race_physical`):
//!
//! - [`RaceFigures`]: las cifras de cada carrera, para las gráficas de evolución. Cada formato se
//!   dibuja aparte, porque un sprint y una larga no se comparan; eso lo decide quien dibuja con
//!   el formato de la carrera.
//! - [`GradeHistory`]: las clases de pendiente con los trozos de todas las carreras juntos.
//! - [`DriftHistory`]: la eficiencia de cada décima relativa a la de su carrera, promediada.
//!
//! Juntar cifras de varias carreras no es promediar sus medias: cada una pesa por el tiempo en
//! que se midió ([`merge_stats`]).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::event_kind::EventKind;
use crate::history::HistoryFilter;
use crate::physical::{BLOCKS, GradeClass, MIN_COVERAGE, RacePhysical, SpeedKind, TransitStats};
use crate::race_format::RaceFormat;

/// Una carrera con FIT, con lo que el filtro del histórico necesita saber de ella.
#[derive(Debug, Clone, Copy)]
pub struct PhysicalRace<'a> {
    pub date: NaiveDate,
    pub format: Option<RaceFormat>,
    pub kind: EventKind,
    pub physical: &'a RacePhysical,
}

/// Las cifras de una carrera para las gráficas de evolución. Cada una es `None` si no se puede
/// dar en esa carrera (sin traslado, sin pulso, sin altitud…).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaceFigures {
    /// Con qué velocidad se compara el traslado de la carrera: la eficiencia y
    /// `transit_speed_mps` van en llano o con la real. Dos carreras de distinto `speed` no se
    /// comparan bien entre sí.
    pub speed: SpeedKind,
    /// Tiempo de traslado (s).
    pub transit_s: f64,
    /// Metros por latido de todo el traslado.
    pub efficiency_m_per_beat: Option<f64>,
    /// Desacople entre mitades (%): positivo si la segunda mitad cuesta más pulso por metro.
    pub decoupling_pct: Option<f64>,
    /// Velocidad del traslado (m/s): en llano si `speed` es `Flat`, y la real si es `Moving`.
    pub transit_speed_mps: Option<f64>,
    /// Pulso medio de la salida a la meta (ppm).
    pub heart_rate_bpm: Option<f64>,
    /// Metros subidos por hora en las subidas.
    pub vam_m_per_h: Option<f64>,
    /// Cadencia media del traslado (pasos/min).
    pub cadence_spm: Option<f64>,
    /// Zancada media del traslado (m por paso).
    pub stride_m: Option<f64>,
    /// Carga de las zonas de pulso de Ajustes.
    pub load: Option<f64>,
}

impl RaceFigures {
    /// Las cifras de una carrera.
    pub fn of(physical: &RacePhysical) -> Self {
        let transit = &physical.transit;
        Self {
            speed: physical.speed,
            transit_s: transit.transit_s,
            efficiency_m_per_beat: transit.efficiency_m_per_beat,
            decoupling_pct: physical.decoupling_pct,
            transit_speed_mps: match physical.speed {
                SpeedKind::Flat => transit.flat_speed_mps,
                SpeedKind::Moving => transit.speed_mps,
            },
            heart_rate_bpm: physical.heart_rate_bpm,
            vam_m_per_h: physical.vam_m_per_h,
            cadence_spm: transit.cadence_spm,
            stride_m: transit.stride_m,
            load: physical.heart_rate_zones.as_ref().map(|z| z.load),
        }
    }
}

/// Una clase de pendiente con los trozos de todas las carreras.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradeHistory {
    pub class: GradeClass,
    /// Carreras con traslado en esta clase.
    pub races: usize,
    #[serde(flatten)]
    pub stats: TransitStats,
}

/// Una décima del traslado con la eficiencia relativa a la de su carrera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriftHistory {
    /// Número de la décima, de 0 a [`BLOCKS`] − 1.
    pub index: usize,
    /// Carreras con eficiencia en esta décima y en todo su traslado.
    pub races: usize,
    /// Media entre carreras de `eficiencia de la décima / eficiencia del traslado entero`
    /// (1 = igual que la media de la carrera); `None` sin carreras.
    pub relative_efficiency: Option<f64>,
}

/// Lo que se agrega entre carreras: las pendientes y las décimas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicalHistory {
    pub filter: HistoryFilter,
    /// Carreras con FIT que pasan el filtro.
    pub races: usize,
    /// De ellas, las que tienen clases de pendiente (todo el traslado con altitud).
    pub graded_races: usize,
    /// Las cinco clases, de más bajada a más subida, siempre las cinco.
    pub by_grade: Vec<GradeHistory>,
    /// Las diez décimas, siempre las diez.
    pub drift: Vec<DriftHistory>,
}

/// Junta las cifras de varios grupos de trozos (la misma clase de pendiente de varias carreras,
/// por ejemplo) como si fueran un solo grupo, pesando cada una por el tiempo en que se midió.
/// Todos deben comparar con la misma velocidad ([`SpeedKind`]) para que la eficiencia tenga
/// sentido.
///
/// - Tiempo y distancia: la suma. Velocidad real: distancia total entre tiempo total.
/// - Velocidad en llano: los metros en llano de cada grupo (su velocidad por su tiempo), sumados,
///   entre el tiempo total; `None` si a algún grupo con traslado le falta.
/// - Pulso: media ponderada por el tiempo con pulso de cada grupo (`heart_rate_s`). Solo se da si
///   ese tiempo suma la mitad del traslado total, como en una carrera.
/// - Eficiencia: metros totales entre latidos totales, es decir, la eficiencia de cada grupo
///   ponderada por sus latidos (pulso por tiempo con pulso).
/// - Cadencia: ponderada por el tiempo con cadencia, con la misma condición que el pulso.
/// - Zancada: metros totales entre pasos totales, es decir, la zancada de cada grupo ponderada
///   por sus pasos (cadencia por tiempo con cadencia).
pub fn merge_stats(parts: &[&TransitStats]) -> TransitStats {
    let transit_s: f64 = parts.iter().map(|p| p.transit_s).sum();
    let distance_m: f64 = parts.iter().map(|p| p.distance_m).sum();
    let per_second = |m: f64| (transit_s > 0.0).then(|| m / transit_s);

    // Los metros en llano de cada grupo son su velocidad por su tiempo.
    let flat_m: Option<f64> = parts
        .iter()
        .filter(|p| p.transit_s > 0.0)
        .map(|p| p.flat_speed_mps.map(|v| v * p.transit_s))
        .sum();

    // Latidos (pulso por tiempo con pulso) y metros comparados (eficiencia por latidos).
    let with_heart_rate: Vec<(&TransitStats, f64, f64)> = parts
        .iter()
        .filter_map(|&p| Some((p, p.heart_rate_bpm?, p.heart_rate_s)))
        .filter(|&(_, _, s)| s > 0.0)
        .collect();
    let heart_rate_s: f64 = with_heart_rate.iter().map(|&(_, _, s)| s).sum();
    let beats: f64 = with_heart_rate
        .iter()
        .map(|&(_, hr, s)| hr * s / 60.0)
        .sum();
    let compared_m: f64 = with_heart_rate
        .iter()
        .filter_map(|&(p, hr, s)| Some(p.efficiency_m_per_beat? * hr * s / 60.0))
        .sum();
    let has_heart_rate = heart_rate_s > 0.0 && heart_rate_s >= MIN_COVERAGE * transit_s;

    // Pasos (cadencia por tiempo con cadencia) y metros recorridos (zancada por pasos).
    let with_cadence: Vec<(&TransitStats, f64, f64)> = parts
        .iter()
        .filter_map(|&p| Some((p, p.cadence_spm?, p.cadence_s)))
        .filter(|&(_, _, s)| s > 0.0)
        .collect();
    let cadence_s: f64 = with_cadence.iter().map(|&(_, _, s)| s).sum();
    let steps: f64 = with_cadence
        .iter()
        .map(|&(_, cadence, s)| cadence * s / 60.0)
        .sum();
    let stepped_m: f64 = with_cadence
        .iter()
        .filter_map(|&(p, cadence, s)| Some(p.stride_m? * cadence * s / 60.0))
        .sum();
    let has_cadence = cadence_s > 0.0 && cadence_s >= MIN_COVERAGE * transit_s;

    TransitStats {
        transit_s,
        distance_m,
        speed_mps: per_second(distance_m),
        flat_speed_mps: flat_m.and_then(per_second),
        heart_rate_bpm: has_heart_rate.then(|| beats * 60.0 / heart_rate_s),
        heart_rate_s: if has_heart_rate { heart_rate_s } else { 0.0 },
        efficiency_m_per_beat: (has_heart_rate && beats > 0.0).then(|| compared_m / beats),
        cadence_spm: has_cadence.then(|| steps * 60.0 / cadence_s),
        cadence_s: if has_cadence { cadence_s } else { 0.0 },
        stride_m: (has_cadence && steps > 0.0).then(|| stepped_m / steps),
    }
}

/// Agrega las carreras que pasan `filter`: las pendientes con los trozos de todas juntas y la
/// deriva media por décimas.
pub fn physical_history(races: &[PhysicalRace<'_>], filter: &HistoryFilter) -> PhysicalHistory {
    let counted: Vec<&RacePhysical> = races
        .iter()
        .filter(|r| filter.includes(r.date, r.format, r.kind))
        .map(|r| r.physical)
        .collect();

    let by_grade = GradeClass::ALL
        .iter()
        .map(|&class| {
            let parts: Vec<&TransitStats> = counted
                .iter()
                .filter_map(|p| p.by_grade.iter().find(|g| g.class == class))
                .map(|g| &g.stats)
                .filter(|s| s.transit_s > 0.0)
                .collect();
            GradeHistory {
                class,
                races: parts.len(),
                stats: merge_stats(&parts),
            }
        })
        .collect();

    // Por décima: suma de las eficiencias relativas y carreras que la aportan.
    let mut tenths = [(0.0, 0usize); BLOCKS];
    for physical in &counted {
        let Some(base) = physical.transit.efficiency_m_per_beat.filter(|&e| e > 0.0) else {
            continue;
        };
        for block in &physical.blocks {
            if let (Some(slot), Some(efficiency)) = (
                tenths.get_mut(block.index),
                block.stats.efficiency_m_per_beat,
            ) {
                slot.0 += efficiency / base;
                slot.1 += 1;
            }
        }
    }
    let drift = tenths
        .iter()
        .enumerate()
        .map(|(index, &(sum, races))| DriftHistory {
            index,
            races,
            relative_efficiency: (races > 0).then(|| sum / races as f64),
        })
        .collect();

    PhysicalHistory {
        filter: *filter,
        races: counted.len(),
        graded_races: counted.iter().filter(|p| !p.by_grade.is_empty()).count(),
        by_grade,
        drift,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physical::{DriftBlock, GradeStats, HeartRateZones};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    /// Un grupo sin nada medido.
    fn empty() -> TransitStats {
        TransitStats {
            transit_s: 0.0,
            distance_m: 0.0,
            speed_mps: None,
            flat_speed_mps: None,
            heart_rate_bpm: None,
            heart_rate_s: 0.0,
            efficiency_m_per_beat: None,
            cadence_spm: None,
            cadence_s: 0.0,
            stride_m: None,
        }
    }

    /// 100 s a 4 m/s reales (400 m) y 5 m/s en llano (500 m en llano). Pulso 150 todo el tiempo:
    /// 250 latidos, así que 500 / 250 = 2 m en llano por latido. Cadencia 180: 300 pasos, de
    /// 400 / 300 = 1,3333 m.
    fn fast() -> TransitStats {
        TransitStats {
            transit_s: 100.0,
            distance_m: 400.0,
            speed_mps: Some(4.0),
            flat_speed_mps: Some(5.0),
            heart_rate_bpm: Some(150.0),
            heart_rate_s: 100.0,
            efficiency_m_per_beat: Some(2.0),
            cadence_spm: Some(180.0),
            cadence_s: 100.0,
            stride_m: Some(400.0 / 300.0),
        }
    }

    /// 300 s a 2 m/s reales (600 m) y 3 m/s en llano (900 m en llano). Pulso 170: 850 latidos,
    /// 900 / 850 m en llano por latido. Sin cadencia.
    fn slow() -> TransitStats {
        TransitStats {
            transit_s: 300.0,
            distance_m: 600.0,
            speed_mps: Some(2.0),
            flat_speed_mps: Some(3.0),
            heart_rate_bpm: Some(170.0),
            heart_rate_s: 300.0,
            efficiency_m_per_beat: Some(900.0 / 850.0),
            ..empty()
        }
    }

    #[test]
    fn merging_weighs_each_group_by_its_time() {
        let merged = merge_stats(&[&fast(), &slow()]);
        assert!(close(merged.transit_s, 400.0));
        assert!(close(merged.distance_m, 1000.0));
        // 1000 m en 400 s: 2,5 m/s. La media de las velocidades (4 y 2) daría 3.
        assert!(close(merged.speed_mps.unwrap(), 2.5));
        // (500 + 900) m en llano en 400 s.
        assert!(close(merged.flat_speed_mps.unwrap(), 3.5));
        // (250 + 850) latidos en 400 s = 165 ppm; (500 + 900) m en llano entre 1100 latidos.
        assert!(close(merged.heart_rate_bpm.unwrap(), 165.0));
        assert!(close(merged.heart_rate_s, 400.0));
        assert!(close(
            merged.efficiency_m_per_beat.unwrap(),
            1400.0 / 1100.0
        ));
        // La cadencia solo está en 100 s de 400: menos de la mitad.
        assert_eq!(merged.cadence_spm, None);
        assert_eq!(merged.stride_m, None);
        assert!(close(merged.cadence_s, 0.0));
    }

    #[test]
    fn merging_needs_half_the_time_with_heart_rate_or_cadence() {
        // El lento sin pulso: el pulso solo cubre 100 s de 400.
        let blind = TransitStats {
            heart_rate_bpm: None,
            heart_rate_s: 0.0,
            efficiency_m_per_beat: None,
            ..slow()
        };
        let merged = merge_stats(&[&fast(), &blind]);
        assert_eq!(merged.heart_rate_bpm, None);
        assert_eq!(merged.efficiency_m_per_beat, None);
        assert!(close(merged.heart_rate_s, 0.0));
        // Con el lento con pulso en la mitad de su tiempo, sí: 250 s de 400.
        let half = TransitStats {
            heart_rate_s: 150.0,
            efficiency_m_per_beat: Some(1.0),
            ..slow()
        };
        let merged = merge_stats(&[&fast(), &half]);
        // Latidos: 250 + 170 · 150 / 60 = 675 en 250 s: 162.
        assert!(close(merged.heart_rate_bpm.unwrap(), 162.0));
        // Metros: 2 · 250 + 1 · 425 = 925 entre 675 latidos.
        assert!(close(merged.efficiency_m_per_beat.unwrap(), 925.0 / 675.0));
        // Cadencia: dos grupos con cadencia y zancada distintas.
        let other = TransitStats {
            cadence_spm: Some(150.0),
            stride_m: Some(1.0),
            ..fast()
        };
        let merged = merge_stats(&[&fast(), &other]);
        // Pasos: 300 + 250 = 550 en 200 s: 165 pasos/min. Metros: 400 + 250 = 650 entre 550.
        assert!(close(merged.cadence_spm.unwrap(), 165.0));
        assert!(close(merged.stride_m.unwrap(), 650.0 / 550.0));
    }

    #[test]
    fn merging_without_altitude_in_a_group_gives_no_flat_speed() {
        let no_altitude = TransitStats {
            flat_speed_mps: None,
            ..slow()
        };
        let merged = merge_stats(&[&fast(), &no_altitude]);
        assert_eq!(merged.flat_speed_mps, None);
        assert!(close(merged.speed_mps.unwrap(), 2.5));
        // Un grupo vacío no cuenta: no tiene traslado.
        let merged = merge_stats(&[&fast(), &empty()]);
        assert!(close(merged.flat_speed_mps.unwrap(), 5.0));
        assert_eq!(merge_stats(&[]), empty());
    }

    /// Una carrera con las cifras de `transit`, las décimas dadas por (índice, eficiencia) y las
    /// clases de pendiente dadas.
    fn race(
        speed: SpeedKind,
        transit: TransitStats,
        tenths: &[(usize, Option<f64>)],
        by_grade: Vec<GradeStats>,
    ) -> RacePhysical {
        RacePhysical {
            duration_s: 700.0,
            distance_m: 2000.0,
            ascent_m: None,
            descent_m: None,
            moving_s: 600.0,
            stopped_s: 0.0,
            gap_s: 0.0,
            attack_s: 0.0,
            speed,
            transit,
            heart_rate_bpm: Some(158.0),
            heart_rate_max_bpm: None,
            halves: None,
            decoupling_pct: Some(4.0),
            speed_change_pct: None,
            heart_rate_change_bpm: None,
            stride_change_pct: None,
            blocks: tenths
                .iter()
                .map(|&(index, efficiency)| DriftBlock {
                    index,
                    from_s: 0.0,
                    to_s: 0.0,
                    stats: TransitStats {
                        efficiency_m_per_beat: efficiency,
                        ..empty()
                    },
                })
                .collect(),
            by_grade,
            vam_m_per_h: Some(900.0),
            climb_s: 120.0,
            heart_rate_zones: Some(HeartRateZones {
                seconds: vec![60.0, 300.0],
                load: 11.0,
            }),
        }
    }

    fn flat_class(stats: TransitStats) -> GradeStats {
        GradeStats {
            class: GradeClass::Flat,
            stats,
        }
    }

    #[test]
    fn figures_use_the_speed_of_the_race() {
        let flat = RaceFigures::of(&race(
            SpeedKind::Flat,
            fast(),
            &[],
            vec![flat_class(fast())],
        ));
        assert_eq!(flat.speed, SpeedKind::Flat);
        assert!(close(flat.transit_s, 100.0));
        assert!(close(flat.transit_speed_mps.unwrap(), 5.0));
        assert!(close(flat.efficiency_m_per_beat.unwrap(), 2.0));
        assert!(close(flat.decoupling_pct.unwrap(), 4.0));
        assert!(close(flat.heart_rate_bpm.unwrap(), 158.0));
        assert!(close(flat.vam_m_per_h.unwrap(), 900.0));
        assert!(close(flat.cadence_spm.unwrap(), 180.0));
        assert!(close(flat.stride_m.unwrap(), 400.0 / 300.0));
        assert!(close(flat.load.unwrap(), 11.0));

        let moving = RaceFigures::of(&race(SpeedKind::Moving, fast(), &[], Vec::new()));
        assert!(close(moving.transit_speed_mps.unwrap(), 4.0));
    }

    #[test]
    fn grades_and_drift_add_up_across_races() {
        // Tres carreras: dos sprints en llano y una larga sin altitud.
        let first = race(
            SpeedKind::Flat,
            TransitStats {
                efficiency_m_per_beat: Some(2.0),
                ..fast()
            },
            &[(0, Some(2.2)), (1, Some(2.0)), (9, Some(1.6))],
            vec![flat_class(fast())],
        );
        let second = race(
            SpeedKind::Flat,
            TransitStats {
                efficiency_m_per_beat: Some(1.0),
                ..slow()
            },
            &[(0, Some(1.2)), (1, None), (9, Some(0.7))],
            vec![flat_class(slow())],
        );
        let third = race(
            SpeedKind::Moving,
            TransitStats {
                efficiency_m_per_beat: Some(1.5),
                ..slow()
            },
            &[(0, Some(1.5))],
            Vec::new(),
        );
        let all = [
            PhysicalRace {
                date: date("2026-01-10"),
                format: Some(RaceFormat::Sprint),
                kind: EventKind::Race,
                physical: &first,
            },
            PhysicalRace {
                date: date("2026-02-10"),
                format: Some(RaceFormat::Sprint),
                kind: EventKind::Race,
                physical: &second,
            },
            PhysicalRace {
                date: date("2026-03-10"),
                format: Some(RaceFormat::Long),
                kind: EventKind::Training,
                physical: &third,
            },
        ];

        let h = physical_history(&all, &HistoryFilter::default());
        assert_eq!(h.races, 3);
        assert_eq!(h.graded_races, 2);
        assert_eq!(h.by_grade.len(), 5);
        let flat = &h.by_grade[2];
        assert_eq!(flat.class, GradeClass::Flat);
        assert_eq!(flat.races, 2);
        assert!(close(flat.stats.transit_s, 400.0));
        assert!(close(flat.stats.speed_mps.unwrap(), 2.5));
        assert!(close(flat.stats.flat_speed_mps.unwrap(), 3.5));
        assert!(close(flat.stats.heart_rate_bpm.unwrap(), 165.0));
        assert!(close(
            flat.stats.efficiency_m_per_beat.unwrap(),
            1400.0 / 1100.0
        ));
        // Las demás clases no tienen traslado.
        for class in h.by_grade.iter().filter(|g| g.class != GradeClass::Flat) {
            assert_eq!(class.races, 0);
            assert_eq!(class.stats.speed_mps, None);
        }

        // Décimas relativas a su carrera: la 0, (2,2/2 + 1,2/1 + 1,5/1,5) / 3 = 1,1; la 1 solo
        // la mide la primera (la segunda no tiene pulso); la 9, (1,6/2 + 0,7/1) / 2 = 0,75.
        assert_eq!(h.drift.len(), BLOCKS);
        assert_eq!(h.drift[0].races, 3);
        assert!(close(h.drift[0].relative_efficiency.unwrap(), 1.1));
        assert_eq!(h.drift[1].races, 1);
        assert!(close(h.drift[1].relative_efficiency.unwrap(), 1.0));
        assert_eq!(h.drift[9].races, 2);
        assert!(close(h.drift[9].relative_efficiency.unwrap(), 0.75));
        assert_eq!(h.drift[5].races, 0);
        assert_eq!(h.drift[5].relative_efficiency, None);

        // El filtro de formato deja fuera la larga; el de tipo, las competiciones.
        let sprints = physical_history(
            &all,
            &HistoryFilter {
                format: Some(RaceFormat::Sprint),
                ..HistoryFilter::default()
            },
        );
        assert_eq!(sprints.races, 2);
        assert!(close(sprints.drift[0].relative_efficiency.unwrap(), 1.15));
        let training = physical_history(
            &all,
            &HistoryFilter {
                kind: Some(EventKind::Training),
                ..HistoryFilter::default()
            },
        );
        assert_eq!(training.races, 1);
        assert_eq!(training.graded_races, 0);
        assert_eq!(training.by_grade[2].races, 0);
        let dated = physical_history(
            &all,
            &HistoryFilter {
                from: Some(date("2026-02-01")),
                to: Some(date("2026-02-28")),
                ..HistoryFilter::default()
            },
        );
        assert_eq!(dated.races, 1);
        assert!(close(dated.by_grade[2].stats.speed_mps.unwrap(), 2.0));
    }

    #[test]
    fn no_races_give_empty_aggregates() {
        let h = physical_history(&[], &HistoryFilter::default());
        assert_eq!(h.races, 0);
        assert_eq!(h.by_grade.len(), 5);
        assert_eq!(h.drift.len(), BLOCKS);
        assert!(h.drift.iter().all(|d| d.relative_efficiency.is_none()));
    }
}
