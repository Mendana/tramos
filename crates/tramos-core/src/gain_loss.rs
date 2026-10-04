//! Dónde gano y dónde pierdo (P5 de `docs/preguntas.md`): la ganancia de cada tramo frente a lo
//! esperado, su acumulado a lo largo de la carrera y las rachas de tramos seguidos perdiendo.
//!
//! Sale de la pérdida de [`crate::lost_time`]: `g_i = esp_i − t_i = −p_i`. Ver
//! `docs/tiempo-perdido.md`, "Dónde gano y dónde pierdo".

use serde::{Deserialize, Serialize};

/// Número mínimo de tramos seguidos perdiendo para que cuenten como racha.
pub const MIN_STREAK_LEGS: usize = 2;

/// Ganancia de un tramo y la acumulada hasta él.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LegGain {
    /// `esp_i − t_i` en segundos: positiva si el tramo fue mejor que el rendimiento habitual.
    pub gain_s: Option<f64>,
    /// Suma de las ganancias conocidas hasta este tramo (incluido); `None` si el tramo no tiene.
    pub cumulative_gain_s: Option<f64>,
}

/// Racha de dos o más tramos seguidos perdiendo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LosingStreak {
    /// Primer y último tramo de la racha (números de tramo, desde 1).
    pub first_leg: usize,
    pub last_leg: usize,
    /// Segundos perdidos en la racha (positivo): la suma de `−g_i` de sus tramos.
    pub loss_s: f64,
}

/// Ganancias de los tramos a partir de sus pérdidas `p_i` (en orden, el tramo `i` en la posición
/// `i − 1`). Un tramo sin pérdida no tiene ganancia ni acumulada, y no suma al acumulado de los
/// siguientes.
pub fn leg_gains(losses: &[Option<f64>]) -> Vec<LegGain> {
    let mut total = 0.0;
    losses
        .iter()
        .map(|loss| {
            // `0 − p` y no `−p`: una pérdida de 0 da ganancia 0, no −0.
            let gain_s = loss.map(|p| 0.0 - p);
            if let Some(g) = gain_s {
                total += g;
            }
            LegGain {
                gain_s,
                cumulative_gain_s: gain_s.map(|_| total),
            }
        })
        .collect()
}

/// Rachas de [`MIN_STREAK_LEGS`] o más tramos seguidos con ganancia estrictamente negativa. Un
/// tramo sin dato corta la racha.
pub fn losing_streaks(gains: &[LegGain]) -> Vec<LosingStreak> {
    let mut streaks = Vec::new();
    let mut run: Vec<(usize, f64)> = Vec::new();
    let mut close = |run: &mut Vec<(usize, f64)>| {
        if run.len() >= MIN_STREAK_LEGS {
            if let (Some(&(first, _)), Some(&(last, _))) = (run.first(), run.last()) {
                streaks.push(LosingStreak {
                    first_leg: first,
                    last_leg: last,
                    loss_s: run.iter().map(|&(_, g)| -g).sum(),
                });
            }
        }
        run.clear();
    };
    for (i, leg) in gains.iter().enumerate() {
        match leg.gain_s {
            Some(g) if g < 0.0 => run.push((i + 1, g)),
            _ => close(&mut run),
        }
    }
    close(&mut run);
    streaks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gains(losses: &[Option<f64>]) -> Vec<LegGain> {
        leg_gains(losses)
    }

    #[test]
    fn gain_is_the_opposite_of_the_loss_and_accumulates() {
        let g = gains(&[Some(5.0), Some(-3.0), Some(10.0)]);
        let values: Vec<_> = g.iter().map(|l| (l.gain_s, l.cumulative_gain_s)).collect();
        assert_eq!(
            values,
            [
                (Some(-5.0), Some(-5.0)),
                (Some(3.0), Some(-2.0)),
                (Some(-10.0), Some(-12.0)),
            ]
        );
    }

    #[test]
    fn a_leg_without_loss_has_no_gain_and_does_not_add() {
        let g = gains(&[Some(4.0), None, Some(-1.0), Some(0.0)]);
        assert_eq!(g[1].gain_s, None);
        assert_eq!(g[1].cumulative_gain_s, None);
        // El acumulado sigue desde el último conocido.
        assert_eq!(g[2].cumulative_gain_s, Some(-3.0));
        assert!(g[3].gain_s.unwrap().is_sign_positive());
    }

    #[test]
    fn streaks_need_two_consecutive_losing_legs() {
        // Pérdidas: tramo 1 pierde solo; 3-5 pierden seguidos; 7-8 también, al final.
        let g = gains(&[
            Some(8.0),
            Some(-2.0),
            Some(1.5),
            Some(20.0),
            Some(0.5),
            Some(-4.0),
            Some(3.0),
            Some(6.0),
        ]);
        assert_eq!(
            losing_streaks(&g),
            [
                LosingStreak {
                    first_leg: 3,
                    last_leg: 5,
                    loss_s: 22.0,
                },
                LosingStreak {
                    first_leg: 7,
                    last_leg: 8,
                    loss_s: 9.0,
                },
            ]
        );
    }

    #[test]
    fn missing_legs_and_exact_expectations_cut_streaks() {
        // Un tramo sin dato o con ganancia 0 no es "perdiendo": corta la racha.
        let g = gains(&[Some(3.0), None, Some(2.0), Some(0.0), Some(1.0)]);
        assert!(losing_streaks(&g).is_empty());
    }

    #[test]
    fn no_legs_no_streaks() {
        assert!(leg_gains(&[]).is_empty());
        assert!(losing_streaks(&[]).is_empty());
    }
}
