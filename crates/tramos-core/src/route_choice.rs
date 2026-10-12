//! Elección de ruta (#190): en un tramo de una carrera con varios atletas, junta los sub-tracks
//! parecidos en **opciones de ruta** y las ordena de la más rápida a la más lenta. Ver
//! `docs/segmentacion.md`, "Elección de ruta".
//!
//! - Cada sub-track se pasa a metros en un plano local y se remuestrea cada
//!   [`RouteOptions::step_m`] a lo largo de su recorrido.
//! - La distancia entre dos rutas es la media de las distancias de cada punto de una a la
//!   polilínea de la otra, en los dos sentidos; se toma la mayor.
//! - Dos sub-tracks son la misma ruta si esa distancia es menor que
//!   [`RouteOptions::tolerance_m`]. Las opciones son las componentes conexas de esa relación.

use serde::{Deserialize, Serialize};

use crate::alignment::median;
use crate::metrics::EARTH_RADIUS_M;
use crate::model::{ControlCode, TrackPoint};

/// Parámetros de la agrupación.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteOptions {
    /// Por debajo de esta distancia media (m), dos sub-tracks son la misma ruta.
    pub tolerance_m: f64,
    /// Paso del remuestreo (m).
    pub step_m: f64,
}

impl Default for RouteOptions {
    fn default() -> Self {
        Self {
            tolerance_m: 30.0,
            step_m: 10.0,
        }
    }
}

/// El sub-track de un atleta en un tramo, con lo que se cuenta de él.
#[derive(Debug, Clone, Copy)]
pub struct RouteTrack<'a> {
    /// El atleta, por su posición en la entrada de quien llama.
    pub runner: usize,
    /// El sub-track del tramo (`docs/segmentacion.md`), de baliza a baliza.
    pub points: &'a [TrackPoint],
    pub split_s: Option<f64>,
    /// Tramo con error de orientación (etiquetado o propuesto): sigue contando, pero se marca.
    pub is_error: bool,
    /// Distancia recorrida (m), de las métricas del tramo.
    pub distance_m: Option<f64>,
    /// Subida acumulada (m), de las métricas del tramo.
    pub ascent_m: Option<f64>,
}

/// Un tramo con más de una opción de ruta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteChoice {
    /// Desde 1.
    pub leg: usize,
    pub from: ControlCode,
    pub to: ControlCode,
    /// Dos o más, de la más rápida a la más lenta por la mediana del split; las que no tienen
    /// ningún split, al final.
    pub options: Vec<RouteOption>,
    /// Mediana de la opción más lenta menos la de la más rápida (s); `None` si menos de dos
    /// opciones tienen split.
    pub spread_s: Option<f64>,
}

/// Una opción de ruta: los atletas que fueron por el mismo sitio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteOption {
    /// Del split más corto al más largo; sin split, al final.
    pub runners: Vec<RouteRunner>,
    pub median_split_s: Option<f64>,
    /// Media de las distancias de sus atletas (m); `None` si ninguno la tiene.
    pub distance_m: Option<f64>,
    /// Media de las subidas de sus atletas (m); `None` si ninguno la tiene.
    pub ascent_m: Option<f64>,
}

/// Un atleta en una opción de ruta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteRunner {
    pub runner: usize,
    pub split_s: Option<f64>,
    pub is_error: bool,
}

/// Las opciones de ruta del tramo `leg` (`from` → `to`) con los sub-tracks de `tracks`. `None`
/// si todos fueron por la misma ruta (o hay menos de dos sub-tracks con dos puntos o más).
pub fn route_choice(
    leg: usize,
    from: ControlCode,
    to: ControlCode,
    tracks: &[RouteTrack<'_>],
    options: &RouteOptions,
) -> Option<RouteChoice> {
    let tracks: Vec<&RouteTrack<'_>> = tracks.iter().filter(|t| t.points.len() >= 2).collect();
    let origin = tracks.first()?.points.first()?;
    let lines: Vec<Vec<Xy>> = tracks
        .iter()
        .map(|t| {
            let line: Vec<Xy> = t.points.iter().map(|p| project(origin, p)).collect();
            resample(&line, options.step_m)
        })
        .collect();

    // Componentes conexas de «misma ruta», con unión-búsqueda.
    let mut parent: Vec<usize> = (0..lines.len()).collect();
    for a in 0..lines.len() {
        for b in a + 1..lines.len() {
            if route_distance(&lines[a], &lines[b]) < options.tolerance_m {
                let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
                parent[ra.max(rb)] = ra.min(rb);
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut group_of: Vec<Option<usize>> = vec![None; lines.len()];
    for i in 0..lines.len() {
        let r = root(&mut parent, i);
        match group_of[r] {
            Some(g) => groups[g].push(i),
            None => {
                group_of[r] = Some(groups.len());
                groups.push(vec![i]);
            }
        }
    }
    if groups.len() < 2 {
        return None;
    }

    let mut route_options: Vec<RouteOption> = groups
        .iter()
        .map(|members| {
            let mut runners: Vec<RouteRunner> = members
                .iter()
                .map(|&i| RouteRunner {
                    runner: tracks[i].runner,
                    split_s: tracks[i].split_s,
                    is_error: tracks[i].is_error,
                })
                .collect();
            runners.sort_by(|a, b| by_split(a.split_s, b.split_s));
            let mean = |values: Vec<f64>| {
                (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
            };
            RouteOption {
                median_split_s: median(runners.iter().filter_map(|r| r.split_s)),
                distance_m: mean(
                    members
                        .iter()
                        .filter_map(|&i| tracks[i].distance_m)
                        .collect(),
                ),
                ascent_m: mean(members.iter().filter_map(|&i| tracks[i].ascent_m).collect()),
                runners,
            }
        })
        .collect();
    route_options.sort_by(|a, b| by_split(a.median_split_s, b.median_split_s));
    let medians: Vec<f64> = route_options
        .iter()
        .filter_map(|o| o.median_split_s)
        .collect();
    let spread_s = match (medians.first(), medians.last()) {
        (Some(fast), Some(slow)) if medians.len() >= 2 => Some(slow - fast),
        _ => None,
    };
    Some(RouteChoice {
        leg,
        from,
        to,
        options: route_options,
        spread_s,
    })
}

/// Del split más corto al más largo; sin split, al final (la ordenación es estable).
fn by_split(a: Option<f64>, b: Option<f64>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.total_cmp(&b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Un punto en metros en el plano local: `x` hacia el este, `y` hacia el norte.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Xy {
    x: f64,
    y: f64,
}

/// Proyección equirrectangular alrededor de `origin`: de sobra para lo que mide un tramo.
fn project(origin: &TrackPoint, p: &TrackPoint) -> Xy {
    let per_degree = EARTH_RADIUS_M * std::f64::consts::PI / 180.0;
    Xy {
        x: (p.lon - origin.lon) * per_degree * origin.lat.to_radians().cos(),
        y: (p.lat - origin.lat) * per_degree,
    }
}

fn distance(a: Xy, b: Xy) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// Puntos cada `step_m` a lo largo de `line`, con el primero y el último.
fn resample(line: &[Xy], step_m: f64) -> Vec<Xy> {
    let Some(&first) = line.first() else {
        return Vec::new();
    };
    let mut out = vec![first];
    // Lo que queda hasta el siguiente punto remuestreado.
    let mut to_next = step_m;
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = distance(a, b);
        let mut along = 0.0;
        while length - along >= to_next {
            along += to_next;
            let f = along / length;
            out.push(Xy {
                x: a.x + f * (b.x - a.x),
                y: a.y + f * (b.y - a.y),
            });
            to_next = step_m;
        }
        to_next -= length - along;
    }
    if let (Some(&last), Some(&end)) = (out.last(), line.last())
        && last != end
    {
        out.push(end);
    }
    out
}

/// La mayor de las dos distancias medias de los puntos de una ruta a la polilínea de la otra.
fn route_distance(a: &[Xy], b: &[Xy]) -> f64 {
    mean_distance(a, b).max(mean_distance(b, a))
}

fn mean_distance(points: &[Xy], line: &[Xy]) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    points.iter().map(|&p| to_polyline(p, line)).sum::<f64>() / points.len() as f64
}

fn to_polyline(p: Xy, line: &[Xy]) -> f64 {
    match line {
        [] => f64::INFINITY,
        [only] => distance(p, *only),
        _ => line
            .windows(2)
            .map(|s| to_segment(p, s[0], s[1]))
            .fold(f64::INFINITY, f64::min),
    }
}

fn to_segment(p: Xy, a: Xy, b: Xy) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length2 = dx * dx + dy * dy;
    if length2 == 0.0 {
        return distance(p, a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / length2).clamp(0.0, 1.0);
    distance(
        p,
        Xy {
            x: a.x + t * dx,
            y: a.y + t * dy,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeDelta, Utc};

    const LAT0: f64 = 43.36;
    const LON0: f64 = -5.85;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    /// Un sub-track que pasa por `vertices` (m al este y al norte de un origen fijo), con un
    /// punto cada 3 m.
    fn track(vertices: &[(f64, f64)]) -> Vec<TrackPoint> {
        let per_degree = EARTH_RADIUS_M * std::f64::consts::PI / 180.0;
        let t0 = DateTime::parse_from_rfc3339("2026-10-03T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut points = Vec::new();
        for pair in vertices.windows(2) {
            let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
            let n = ((x1 - x0).hypot(y1 - y0) / 3.0).ceil() as usize;
            for k in 0..n {
                let f = k as f64 / n as f64;
                points.push((x0 + f * (x1 - x0), y0 + f * (y1 - y0)));
            }
        }
        points.push(*vertices.last().unwrap());
        points
            .into_iter()
            .enumerate()
            .map(|(i, (x, y))| TrackPoint {
                time: t0 + TimeDelta::seconds(i as i64),
                lat: LAT0 + y / per_degree,
                lon: LON0 + x / (per_degree * LAT0.to_radians().cos()),
                altitude_m: None,
                heart_rate_bpm: None,
                cadence_spm: None,
                distance_m: None,
            })
            .collect()
    }

    fn input(runner: usize, points: &[TrackPoint], split_s: f64) -> RouteTrack<'_> {
        RouteTrack {
            runner,
            points,
            split_s: Some(split_s),
            is_error: false,
            distance_m: None,
            ascent_m: None,
        }
    }

    fn line(points: &[TrackPoint]) -> Vec<Xy> {
        // El origen, común a todas: el de `track`.
        let origin = &track(&[(0.0, 0.0)])[0];
        let xy: Vec<Xy> = points.iter().map(|p| project(origin, p)).collect();
        resample(&xy, 10.0)
    }

    #[test]
    fn resampling_keeps_both_ends_every_step() {
        let xy = [Xy { x: 0.0, y: 0.0 }, Xy { x: 0.0, y: 25.0 }];
        let out = resample(&xy, 10.0);
        let ys: Vec<f64> = out.iter().map(|p| p.y).collect();
        assert_eq!(ys, [0.0, 10.0, 20.0, 25.0]);
        // El paso sigue a lo largo de la línea, aunque doble una esquina.
        let corner = [
            Xy { x: 0.0, y: 0.0 },
            Xy { x: 0.0, y: 15.0 },
            Xy { x: 15.0, y: 15.0 },
        ];
        let out = resample(&corner, 10.0);
        assert_eq!(out.len(), 4);
        assert!(close(out[2].x, 5.0, 1e-9) && close(out[2].y, 15.0, 1e-9));
        assert_eq!(out[3], corner[2]);
    }

    #[test]
    fn a_parallel_route_is_as_far_as_its_offset() {
        // De (0, 0) a (−200, 500): desplazada 10 m al este, cada punto queda a
        // 10 · 500 / √(200² + 500²) = 9,285 m de la otra línea.
        let west = track(&[(0.0, 0.0), (-200.0, 500.0)]);
        let shifted = track(&[(10.0, 0.0), (-190.0, 500.0)]);
        let d = route_distance(&line(&west), &line(&shifted));
        let expected = 10.0 * 500.0 / 200.0f64.hypot(500.0);
        assert!(close(d, expected, 0.05), "{d} frente a {expected}");
    }

    /// De la baliza (0, 0) a la (0, 1000), con un obstáculo en medio: por el oeste o por el
    /// este, a 200 m de la línea recta.
    fn two_sides() -> [Vec<TrackPoint>; 4] {
        [
            track(&[(0.0, 0.0), (-200.0, 500.0), (0.0, 1000.0)]),
            track(&[(0.0, 0.0), (200.0, 500.0), (0.0, 1000.0)]),
            // Unos 10 m más afuera que la del oeste: la misma opción.
            track(&[
                (0.0, 0.0),
                (-10.0, 15.0),
                (-210.0, 500.0),
                (-10.0, 985.0),
                (0.0, 1000.0),
            ]),
            track(&[(0.0, 0.0), (205.0, 520.0), (0.0, 1000.0)]),
        ]
    }

    #[test]
    fn two_sides_of_an_obstacle_are_two_options() {
        let [west, east, west_wide, east_late] = two_sides();
        let mut tracks = vec![
            input(0, &west, 260.0),
            input(1, &east, 230.0),
            input(2, &west_wide, 250.0),
            input(3, &east_late, 250.0),
        ];
        tracks[0].distance_m = Some(1080.0);
        tracks[2].distance_m = Some(1100.0);
        tracks[2].ascent_m = Some(30.0);
        tracks[2].is_error = true;
        let choice = route_choice(4, 31, 32, &tracks, &RouteOptions::default()).unwrap();
        assert_eq!((choice.leg, choice.from, choice.to), (4, 31, 32));
        assert_eq!(choice.options.len(), 2);

        // El este, más rápido: mediana de 230 y 250.
        let east = &choice.options[0];
        let who: Vec<usize> = east.runners.iter().map(|r| r.runner).collect();
        assert_eq!(who, [1, 3]);
        assert_eq!(east.median_split_s, Some(240.0));
        assert_eq!((east.distance_m, east.ascent_m), (None, None));

        // El oeste, del más rápido al más lento; el que falló sigue contando, marcado.
        let west = &choice.options[1];
        let who: Vec<(usize, bool)> = west
            .runners
            .iter()
            .map(|r| (r.runner, r.is_error))
            .collect();
        assert_eq!(who, [(2, true), (0, false)]);
        assert_eq!(west.median_split_s, Some(255.0));
        assert_eq!(west.distance_m, Some(1090.0));
        assert_eq!(west.ascent_m, Some(30.0));

        assert_eq!(choice.spread_s, Some(15.0));
    }

    #[test]
    fn a_straight_leg_has_no_choice() {
        let straight = track(&[(0.0, 0.0), (0.0, 600.0)]);
        let beside = track(&[(10.0, 0.0), (10.0, 300.0), (0.0, 600.0)]);
        let tracks = [input(0, &straight, 120.0), input(1, &beside, 125.0)];
        assert_eq!(
            route_choice(1, 31, 32, &tracks, &RouteOptions::default()),
            None
        );
        // Con un solo sub-track (o ninguno con dos puntos), tampoco.
        assert_eq!(
            route_choice(1, 31, 32, &tracks[..1], &RouteOptions::default()),
            None
        );
        let dot = [straight[0].clone()];
        let tracks = [input(0, &straight, 120.0), input(1, &dot, 125.0)];
        assert_eq!(
            route_choice(1, 31, 32, &tracks, &RouteOptions::default()),
            None
        );
    }

    #[test]
    fn a_chain_of_close_routes_is_one_option() {
        // Cada una 50 m más afuera que la anterior en el punto más lejano: cerca de la siguiente,
        // pero la primera y la última, lejos. Las une la del medio (componentes conexas).
        let a = track(&[(0.0, 0.0), (-200.0, 500.0), (0.0, 1000.0)]);
        let b = track(&[(0.0, 0.0), (-250.0, 500.0), (0.0, 1000.0)]);
        let c = track(&[(0.0, 0.0), (-300.0, 500.0), (0.0, 1000.0)]);
        let d = track(&[(0.0, 0.0), (200.0, 500.0), (0.0, 1000.0)]);
        let options = RouteOptions::default();
        assert!(route_distance(&line(&a), &line(&b)) < options.tolerance_m);
        assert!(route_distance(&line(&b), &line(&c)) < options.tolerance_m);
        assert!(route_distance(&line(&a), &line(&c)) > options.tolerance_m);
        let tracks = [
            input(0, &a, 200.0),
            input(1, &d, 210.0),
            input(2, &c, 220.0),
            input(3, &b, 230.0),
        ];
        let choice = route_choice(2, 31, 32, &tracks, &options).unwrap();
        let who: Vec<Vec<usize>> = choice
            .options
            .iter()
            .map(|o| o.runners.iter().map(|r| r.runner).collect())
            .collect();
        assert_eq!(who, [vec![1], vec![0, 2, 3]]);
        // 210 frente a la mediana de 200, 220 y 230.
        assert_eq!(choice.spread_s, Some(10.0));
    }

    #[test]
    fn options_without_splits_go_last_and_do_not_count_for_the_spread() {
        let [west, east, ..] = two_sides();
        let mut tracks = [input(0, &west, 0.0), input(1, &east, 230.0)];
        tracks[0].split_s = None;
        let choice = route_choice(3, 31, 32, &tracks, &RouteOptions::default()).unwrap();
        assert_eq!(choice.options[0].runners[0].runner, 1);
        assert_eq!(choice.options[1].median_split_s, None);
        assert_eq!(choice.spread_s, None);
    }
}
