//! Núcleo de Tramos: modelos, importadores (.spl, FIT), alineación, segmentación,
//! métricas y análisis. Sin dependencias de interfaz.

pub mod after_error;
pub mod alignment;
pub mod comparison;
pub mod consistency;
pub mod courses;
pub mod days_off;
pub mod gain_loss;
pub mod history;
pub mod identify;
pub mod importers;
pub mod leg_length;
pub mod loss_breakdown;
pub mod lost_time;
pub mod metrics;
pub mod model;
pub mod race_format;
pub mod runner_report;
pub mod segmentation;
pub mod slope;

/// Versión del núcleo, tomada de `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_manifest() {
        assert_eq!(VERSION, "0.1.0");
    }
}
