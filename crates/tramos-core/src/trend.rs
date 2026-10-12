//! ¿Estoy mejorando? (#192): la tendencia del rendimiento habitual (IR) y de la tasa de error a
//! lo largo de las carreras del histórico, con su incertidumbre. Las definiciones están en
//! `docs/historico.md`, "¿Estoy mejorando?".
//!
//! Una recta por mínimos cuadrados sobre la fecha de cada carrera (la de la tasa de error,
//! ponderada por sus tramos) y el intervalo al 90 % de su pendiente con la t de Student. Solo hay
//! veredicto con bastantes carreras, y solo dice «mejoras» o «empeoras» si el intervalo no cruza
//! el cero: con pocas carreras y mucho ruido, lo honesto es no decir nada.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::history::{HistoryFilter, HistoryRace, race_stats};

/// Carreras mínimas para dar veredicto.
pub const MIN_RACES: usize = 8;
/// Días mínimos entre la primera y la última carrera para dar veredicto (unos 3 meses).
pub const MIN_SPAN_DAYS: i64 = 90;
/// La pendiente se da por cada 3 meses (días).
pub const QUARTER_DAYS: f64 = 365.25 / 4.0;
/// Nivel del intervalo de la pendiente y de la banda de la recta.
pub const CONFIDENCE: f64 = 0.90;

/// Qué dice la tendencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrendVerdict {
    /// Mejora: el intervalo de la pendiente queda entero del lado bueno.
    Improving,
    /// Empeora: entero del lado malo.
    Worsening,
    /// El intervalo cruza el cero: no se puede decir que cambie.
    NoChange,
    /// Menos de [`MIN_RACES`] carreras o de [`MIN_SPAN_DAYS`] días.
    NotEnoughData,
}

/// Una carrera de la serie, con la recta y su banda en su fecha.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrendPoint {
    pub date: NaiveDate,
    /// El valor de la carrera (IR habitual o tasa de error, 1 = 100 %).
    pub value: f64,
    /// Su peso en la recta: 1 en el IR; los tramos que cuentan en la tasa de error.
    pub weight: f64,
    /// La recta en esa fecha y su banda al 90 %; `None` sin recta.
    pub fitted: Option<f64>,
    pub low: Option<f64>,
    pub high: Option<f64>,
}

/// La tendencia de una serie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trend {
    /// Las carreras, de la más antigua a la más reciente.
    pub points: Vec<TrendPoint>,
    /// Días entre la primera y la última.
    pub span_days: i64,
    /// Pendiente por cada 3 meses (1 = 100 puntos) y su intervalo al 90 %; `None` con menos de
    /// tres carreras o todas el mismo día.
    pub slope_per_quarter: Option<f64>,
    pub slope_low: Option<f64>,
    pub slope_high: Option<f64>,
    pub verdict: TrendVerdict,
    /// Carreras que faltan para el mínimo (0 si no faltan).
    pub missing_races: usize,
    /// Días que faltan para el mínimo (0 si no faltan).
    pub missing_days: i64,
}

/// Las dos tendencias.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trends {
    /// Rendimiento habitual: más alto es mejor.
    pub performance: Trend,
    /// Tasa de error de los tramos que cuentan: más baja es mejor.
    pub error_rate: Trend,
}

/// Las tendencias de las carreras que pasan `filter` y tienen rendimiento habitual (las mismas
/// que [`crate::history::history`]). En la tasa de error, solo las que tienen tramos que cuentan.
pub fn trends(races: &[HistoryRace], filter: &HistoryFilter) -> Trends {
    let mut counted: Vec<&HistoryRace> = races
        .iter()
        .filter(|r| filter.includes_race(r))
        .filter(|r| r.lost_time.usual_performance.is_some())
        .collect();
    counted.sort_by_key(|r| r.date);
    let performance: Vec<(NaiveDate, f64, f64)> = counted
        .iter()
        .filter_map(|r| Some((r.date, r.lost_time.usual_performance?, 1.0)))
        .collect();
    let error_rate: Vec<(NaiveDate, f64, f64)> = counted
        .iter()
        .filter_map(|r| {
            let stats = race_stats(&r.lost_time)?;
            Some((r.date, stats.error_rate?, stats.legs as f64))
        })
        .filter(|&(_, _, legs)| legs > 0.0)
        .collect();
    Trends {
        performance: trend(&performance, true),
        error_rate: trend(&error_rate, false),
    }
}

/// La tendencia de `(fecha, valor, peso)` en orden de fecha. `higher_is_better` dice qué lado
/// de la pendiente es mejorar.
pub fn trend(series: &[(NaiveDate, f64, f64)], higher_is_better: bool) -> Trend {
    let first = series.first().map(|p| p.0);
    let day = |d: NaiveDate| first.map_or(0.0, |f| (d - f).num_days() as f64);
    let span_days = match (series.first(), series.last()) {
        (Some(a), Some(b)) => (b.0 - a.0).num_days(),
        _ => 0,
    };
    let xyw: Vec<(f64, f64, f64)> = series.iter().map(|&(d, y, w)| (day(d), y, w)).collect();
    let fit = Fit::of(&xyw);
    let t = fit
        .as_ref()
        .and_then(|f| student_t_quantile(0.5 + CONFIDENCE / 2.0, f.df()));

    let points = series
        .iter()
        .map(|&(date, value, weight)| {
            let x = day(date);
            let band = fit.as_ref().zip(t).map(|(f, t)| {
                let y = f.at(x);
                let half = t * f.se_at(x);
                (y, y - half, y + half)
            });
            TrendPoint {
                date,
                value,
                weight,
                fitted: band.map(|b| b.0),
                low: band.map(|b| b.1),
                high: band.map(|b| b.2),
            }
        })
        .collect();

    let slope = fit.as_ref().zip(t).map(|(f, t)| {
        let half = t * f.se_slope();
        (
            f.slope * QUARTER_DAYS,
            (f.slope - half) * QUARTER_DAYS,
            (f.slope + half) * QUARTER_DAYS,
        )
    });
    let missing_races = MIN_RACES.saturating_sub(series.len());
    let missing_days = (MIN_SPAN_DAYS - span_days).max(0);
    let verdict = match slope {
        _ if missing_races > 0 || missing_days > 0 => TrendVerdict::NotEnoughData,
        None => TrendVerdict::NotEnoughData,
        Some((_, low, high)) => {
            let (good, bad) = if higher_is_better {
                (low > 0.0, high < 0.0)
            } else {
                (high < 0.0, low > 0.0)
            };
            if good {
                TrendVerdict::Improving
            } else if bad {
                TrendVerdict::Worsening
            } else {
                TrendVerdict::NoChange
            }
        }
    };
    Trend {
        points,
        span_days,
        slope_per_quarter: slope.map(|s| s.0),
        slope_low: slope.map(|s| s.1),
        slope_high: slope.map(|s| s.2),
        verdict,
        missing_races,
        missing_days,
    }
}

/// Recta por mínimos cuadrados ponderados.
struct Fit {
    slope: f64,
    intercept: f64,
    /// Media ponderada de x, suma de pesos y `Σ w (x − x̄)²`.
    x_mean: f64,
    weight: f64,
    sxx: f64,
    /// Varianza residual por unidad de peso: `Σ w r² / (n − 2)`.
    s2: f64,
    n: usize,
}

impl Fit {
    /// `None` con menos de tres puntos, pesos que no suman algo positivo o todas las x iguales.
    fn of(points: &[(f64, f64, f64)]) -> Option<Self> {
        let n = points.len();
        let weight: f64 = points.iter().map(|p| p.2).sum();
        if n < 3 || weight <= 0.0 {
            return None;
        }
        let x_mean = points.iter().map(|&(x, _, w)| w * x).sum::<f64>() / weight;
        let y_mean = points.iter().map(|&(_, y, w)| w * y).sum::<f64>() / weight;
        let sxx: f64 = points
            .iter()
            .map(|&(x, _, w)| w * (x - x_mean).powi(2))
            .sum();
        if sxx <= 0.0 {
            return None;
        }
        let sxy: f64 = points
            .iter()
            .map(|&(x, y, w)| w * (x - x_mean) * (y - y_mean))
            .sum();
        let slope = sxy / sxx;
        let intercept = y_mean - slope * x_mean;
        let rss: f64 = points
            .iter()
            .map(|&(x, y, w)| w * (y - intercept - slope * x).powi(2))
            .sum();
        Some(Fit {
            slope,
            intercept,
            x_mean,
            weight,
            sxx,
            s2: rss / (n - 2) as f64,
            n,
        })
    }

    fn df(&self) -> f64 {
        (self.n - 2) as f64
    }

    fn at(&self, x: f64) -> f64 {
        self.intercept + self.slope * x
    }

    fn se_slope(&self) -> f64 {
        (self.s2 / self.sxx).sqrt()
    }

    /// Error típico de la recta (la media, no una carrera nueva) en `x`.
    fn se_at(&self, x: f64) -> f64 {
        (self.s2 * (1.0 / self.weight + (x - self.x_mean).powi(2) / self.sxx)).sqrt()
    }
}

/// Cuantil `p` (entre 0,5 y 1) de la t de Student con `df` grados de libertad, por bisección
/// sobre su función de distribución. `None` fuera de rango.
pub fn student_t_quantile(p: f64, df: f64) -> Option<f64> {
    if !(0.5..1.0).contains(&p) || df.is_nan() || df <= 0.0 {
        return None;
    }
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    while student_t_cdf(hi, df) < p {
        hi *= 2.0;
        if hi > 1e6 {
            return None;
        }
    }
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        if student_t_cdf(mid, df) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some((lo + hi) / 2.0)
}

/// Función de distribución de la t de Student: `1 − I_x(df/2, 1/2) / 2` para `t ≥ 0`, con
/// `x = df / (df + t²)` y la beta incompleta regularizada.
fn student_t_cdf(t: f64, df: f64) -> f64 {
    let tail = 0.5 * incomplete_beta(df / (df + t * t), df / 2.0, 0.5);
    if t >= 0.0 { 1.0 - tail } else { tail }
}

/// Beta incompleta regularizada `I_x(a, b)` por su fracción continua (Numerical Recipes, `betai`).
fn incomplete_beta(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let front =
        (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_fraction(x, a, b) / a
    } else {
        1.0 - front * beta_fraction(1.0 - x, b, a) / b
    }
}

/// Fracción continua de la beta incompleta (método de Lentz).
fn beta_fraction(x: f64, a: f64, b: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..300 {
        let m = f64::from(m);
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < 1e-14 {
            break;
        }
    }
    h
}

/// Logaritmo de la función gamma (aproximación de Lanczos, g = 7), para `x > 0`.
fn ln_gamma(x: f64) -> f64 {
    const COEFFICIENTS: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        // Reflexión: Γ(x) Γ(1 − x) = π / sin(πx).
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut sum = COEFFICIENTS[0];
    for (i, c) in COEFFICIENTS.iter().enumerate().skip(1) {
        sum += c / (x + i as f64);
    }
    let t = x + 7.5;
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + sum.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) {
        assert!((a - b).abs() < eps, "{a} != {b}");
    }

    fn day(n: i64) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() + chrono::TimeDelta::days(n)
    }

    #[test]
    fn t_quantiles_match_the_tables() {
        // t de dos colas al 90 % (cuantil 0,95), de las tablas.
        close(student_t_quantile(0.95, 1.0).unwrap(), 6.314, 1e-3);
        close(student_t_quantile(0.95, 6.0).unwrap(), 1.943, 1e-3);
        close(student_t_quantile(0.95, 10.0).unwrap(), 1.812, 1e-3);
        close(student_t_quantile(0.95, 30.0).unwrap(), 1.697, 1e-3);
        close(student_t_quantile(0.975, 4.0).unwrap(), 2.776, 1e-3);
        close(student_t_quantile(0.5, 5.0).unwrap(), 0.0, 1e-9);
        assert_eq!(student_t_quantile(1.0, 5.0), None);
    }

    #[test]
    fn ln_gamma_of_integers_is_the_log_factorial() {
        close(ln_gamma(1.0), 0.0, 1e-12);
        close(ln_gamma(5.0), 24.0_f64.ln(), 1e-12);
        close(ln_gamma(0.5), std::f64::consts::PI.sqrt().ln(), 1e-12);
    }

    #[test]
    fn a_clear_rise_is_improving_and_its_interval_is_computed_by_hand() {
        // 8 carreras cada 15 días (105 días) con el IR subiendo 0,3 puntos por día y un ruido
        // alterno de ±1 punto: y = 0,80 + 0,003·x ± 0,01.
        let series: Vec<(NaiveDate, f64, f64)> = (0..8)
            .map(|i| {
                let x = 15 * i;
                let noise = if i % 2 == 0 { 0.01 } else { -0.01 };
                (day(x), 0.80 + 0.003 * x as f64 + noise, 1.0)
            })
            .collect();
        let t = trend(&series, true);
        assert_eq!(t.verdict, TrendVerdict::Improving);
        assert_eq!(t.span_days, 105);
        assert_eq!((t.missing_races, t.missing_days), (0, 0));
        // Mínimos cuadrados a mano: x̄ = 52,5 y Sxx = 9450; el intervalo, con t(0,95; 6) =
        // 1,943 de las tablas.
        let xs: Vec<f64> = (0..8).map(|i| 15.0 * i as f64).collect();
        let ys: Vec<f64> = series.iter().map(|p| p.1).collect();
        let xm = xs.iter().sum::<f64>() / 8.0;
        let ym = ys.iter().sum::<f64>() / 8.0;
        let sxx: f64 = xs.iter().map(|x| (x - xm).powi(2)).sum();
        close(sxx, 9450.0, 1e-9);
        let sxy: f64 = xs.iter().zip(&ys).map(|(x, y)| (x - xm) * (y - ym)).sum();
        let b = sxy / sxx;
        let a = ym - b * xm;
        let rss: f64 = xs
            .iter()
            .zip(&ys)
            .map(|(x, y)| (y - a - b * x).powi(2))
            .sum();
        let half = 1.943_180_281 * (rss / 6.0 / sxx).sqrt();
        close(t.slope_per_quarter.unwrap(), b * QUARTER_DAYS, 1e-9);
        close(t.slope_low.unwrap(), (b - half) * QUARTER_DAYS, 1e-6);
        close(t.slope_high.unwrap(), (b + half) * QUARTER_DAYS, 1e-6);
        // Unos 27 puntos por trimestre.
        close(t.slope_per_quarter.unwrap(), 0.274, 0.01);
        // La recta pasa por la media en x̄, y la banda es más estrecha en el centro.
        let p = t.points[0];
        close(p.fitted.unwrap(), a, 1e-9);
        let width = |p: &TrendPoint| p.high.unwrap() - p.low.unwrap();
        assert!(width(&t.points[3]) < width(&t.points[0]));
    }

    #[test]
    fn noise_without_a_trend_is_no_change() {
        // Ruido grande alterno sin tendencia.
        let series: Vec<(NaiveDate, f64, f64)> = (0..10)
            .map(|i| (day(20 * i), if i % 2 == 0 { 0.95 } else { 0.80 }, 1.0))
            .collect();
        assert_eq!(trend(&series, true).verdict, TrendVerdict::NoChange);
    }

    #[test]
    fn for_the_error_rate_a_fall_is_improving() {
        let series: Vec<(NaiveDate, f64, f64)> = (0..8)
            .map(|i| {
                (
                    day(15 * i),
                    0.40 - 0.03 * i as f64 + if i % 2 == 0 { 0.01 } else { 0.0 },
                    10.0,
                )
            })
            .collect();
        assert_eq!(trend(&series, false).verdict, TrendVerdict::Improving);
        assert_eq!(trend(&series, true).verdict, TrendVerdict::Worsening);
    }

    #[test]
    fn weights_pull_the_line() {
        // Tres puntos en línea y uno fuera: con poco peso apenas mueve la pendiente.
        let base = [(0.0, 0.1, 10.0), (10.0, 0.2, 10.0), (20.0, 0.3, 10.0)];
        let with = |w: f64| {
            let mut p = base.to_vec();
            p.push((30.0, 0.0, w));
            Fit::of(&p).unwrap().slope
        };
        assert!((with(0.1) - 0.01).abs() < (with(10.0) - 0.01).abs());
    }

    #[test]
    fn few_races_or_a_short_span_give_no_verdict() {
        // 5 carreras: faltan 3, aunque la tendencia sea clara.
        let series: Vec<(NaiveDate, f64, f64)> = (0..5)
            .map(|i| (day(30 * i), 0.8 + 0.05 * i as f64, 1.0))
            .collect();
        let t = trend(&series, true);
        assert_eq!(t.verdict, TrendVerdict::NotEnoughData);
        assert_eq!(t.missing_races, 3);
        assert!(t.slope_per_quarter.is_some());
        // 8 carreras en 35 días: faltan 55 días.
        let series: Vec<(NaiveDate, f64, f64)> = (0..8)
            .map(|i| (day(5 * i), 0.8 + 0.05 * i as f64, 1.0))
            .collect();
        let t = trend(&series, true);
        assert_eq!(t.verdict, TrendVerdict::NotEnoughData);
        assert_eq!((t.missing_races, t.missing_days), (0, 55));
        // Dos carreras: ni recta.
        let t = trend(&series[..2], true);
        assert_eq!(t.slope_per_quarter, None);
        assert!(t.points.iter().all(|p| p.fitted.is_none()));
    }

    /// Carrera con rendimiento habitual `usual` y `legs` tramos que cuentan, `errors` de ellos
    /// con error (más un último que no cuenta).
    fn race(date: NaiveDate, usual: Option<f64>, legs: usize, errors: usize) -> HistoryRace {
        use crate::runner_report::{LegReport, RunnerLostTime};
        let leg = |i: usize| LegReport {
            index: i + 1,
            from: 31,
            to: 32,
            split_s: Some(60.0),
            elapsed_s: None,
            place: None,
            reference_s: Some(60.0),
            reference_count: 1,
            valid_splits: 4,
            performance_index: Some(1.0),
            expected_s: Some(60.0),
            loss_s: Some(if i < errors { 30.0 } else { 0.0 }),
            loss_pct: Some(0.0),
            is_error: i < errors,
            gain_s: None,
            cumulative_gain_s: None,
            ideal_elapsed_s: None,
            behind_ideal_s: None,
            is_last: i == legs,
            short_reference: false,
            excluded_from_patterns: i == legs,
        };
        HistoryRace {
            date,
            format: None,
            kind: crate::event_kind::EventKind::Race,
            lost_time: RunnerLostTime {
                total_s: None,
                usual_performance: usual,
                lost_time_s: None,
                error_count: errors,
                time_without_errors_s: None,
                place_without_errors: None,
                ideal_time_s: None,
                behind_ideal_s: None,
                losing_streaks: Vec::new(),
                consistency: None,
                legs: (0..=legs).map(leg).collect(),
            },
        }
    }

    #[test]
    fn trends_take_the_races_of_the_history_in_date_order() {
        let mut races: Vec<HistoryRace> = (0..8)
            .map(|i| race(day(15 * i), Some(0.8 + 0.01 * i as f64), 10, 4))
            .collect();
        // Desordenadas, una sin rendimiento habitual y una fuera del filtro de fechas.
        races.reverse();
        races.push(race(day(3), None, 10, 1));
        races.push(race(day(400), Some(0.1), 10, 9));
        let filter = HistoryFilter {
            to: Some(day(200)),
            ..HistoryFilter::default()
        };
        let t = trends(&races, &filter);
        assert_eq!(t.performance.points.len(), 8);
        assert_eq!(t.performance.points[0].date, day(0));
        close(t.performance.points[7].value, 0.87, 1e-9);
        // La tasa de error, 4 de 10 en cada una, pesa por sus tramos que cuentan.
        assert_eq!(t.error_rate.points.len(), 8);
        close(t.error_rate.points[0].value, 0.4, 1e-9);
        close(t.error_rate.points[0].weight, 10.0, 1e-9);
        // Sin ruido, la recta es exacta: el intervalo se cierra sobre la pendiente.
        assert_eq!(t.performance.verdict, TrendVerdict::Improving);
        close(t.error_rate.slope_per_quarter.unwrap(), 0.0, 1e-12);
    }
}
