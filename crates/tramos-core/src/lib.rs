//! Núcleo de Tramos: modelos, importadores (.spl, FIT), alineación, segmentación,
//! métricas y análisis. Sin dependencias de interfaz.

pub mod alignment;
pub mod courses;
pub mod identify;
pub mod importers;
pub mod lost_time;
pub mod metrics;
pub mod model;
pub mod race_format;
pub mod segmentation;

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
