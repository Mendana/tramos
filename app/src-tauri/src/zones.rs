//! Zonas de color del mapa (#96): intervalos fijos de ritmo o de pulso, con el color de cada uno,
//! que el usuario define en Ajustes en vez de las clases por cuantiles de cada carrera.
//!
//! Aquí están el formato, a qué zona va cada valor y los avisos sobre los colores (que se lean
//! sobre el mapa y que dos zonas seguidas se distingan). `docs/app.md`, "Mapa" y "Ajustes".

use serde::{Deserialize, Serialize};

/// Zonas como mucho.
pub const MAX_ZONES: usize = 10;
/// Fondo de las teselas de OpenStreetMap (`--map-paper` en `tokens.css`).
pub const MAP_PAPER: &str = "#f2efe9";
/// Contraste mínimo (WCAG) de un color con [`MAP_PAPER`]; el tono más claro de la escala por
/// defecto tiene 2,18.
pub const MIN_CONTRAST: f64 = 2.0;
/// Distancia mínima (OKLab × 100) entre los colores de dos zonas seguidas; en la escala por
/// defecto, los tonos seguidos están a entre 9,5 y 10,4.
pub const MIN_DISTANCE: f64 = 9.0;

/// Zonas de una magnitud (ritmo en s/km o pulso en ppm), de menor a mayor valor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Zones {
    /// Límites entre zonas, de menor a mayor: uno menos que colores. La zona `k` va de
    /// `limits[k − 1]` (incluido) a `limits[k]` (sin incluir); la primera no tiene mínimo y la
    /// última, máximo.
    pub limits: Vec<f64>,
    /// Color de cada zona, `#rrggbb`.
    pub colors: Vec<String>,
}

impl Zones {
    /// Zona de `value`: cuántos límites alcanza. Un valor justo en un límite va a la zona de
    /// arriba («< 120», «120–140», «≥ 140»).
    pub fn zone(&self, value: f64) -> usize {
        self.limits.iter().filter(|&&limit| value >= limit).count()
    }

    /// Por qué no valen, en español; `None` si valen.
    pub fn problem(&self) -> Option<String> {
        if self.colors.len() < 2 {
            return Some("hacen falta al menos dos zonas".into());
        }
        if self.colors.len() > MAX_ZONES {
            return Some(format!("como mucho {MAX_ZONES} zonas"));
        }
        if self.limits.len() + 1 != self.colors.len() {
            return Some("cada zona, menos la primera, necesita su límite".into());
        }
        if self.limits.iter().any(|l| !l.is_finite() || *l <= 0.0) {
            return Some("los límites tienen que ser números mayores que 0".into());
        }
        if self.limits.windows(2).any(|w| w[0] >= w[1]) {
            return Some("los límites tienen que ir de menor a mayor, sin repetirse".into());
        }
        if let Some(bad) = self.colors.iter().find(|c| rgb(c).is_none()) {
            return Some(format!("{bad:?} no es un color #rrggbb"));
        }
        None
    }

    /// Avisos sobre los colores, en español: zonas que se ven poco sobre el mapa y zonas
    /// seguidas que se confunden. No impiden guardarlas.
    pub fn warnings(&self) -> Vec<String> {
        let Some(paper) = rgb(MAP_PAPER) else {
            return Vec::new();
        };
        let colors: Vec<Option<[u8; 3]>> = self.colors.iter().map(|c| rgb(c)).collect();
        let mut warnings = Vec::new();
        for (k, color) in colors.iter().enumerate() {
            if color.is_some_and(|c| contrast(c, paper) < MIN_CONTRAST) {
                warnings.push(format!(
                    "La zona {} se ve poco sobre el mapa: elige un color más oscuro o más intenso.",
                    k + 1
                ));
            }
        }
        for (k, pair) in colors.windows(2).enumerate() {
            if let [Some(a), Some(b)] = pair
                && distance(*a, *b) < MIN_DISTANCE
            {
                warnings.push(format!(
                    "Las zonas {} y {} tienen colores muy parecidos: cuesta distinguirlas.",
                    k + 1,
                    k + 2
                ));
            }
        }
        warnings
    }

    /// Las zonas guardadas en un ajuste (JSON); `None` si no hay o no valen.
    pub fn from_setting(value: Option<&str>) -> Option<Self> {
        let zones: Self = serde_json::from_str(value?).ok()?;
        zones.problem().is_none().then_some(zones)
    }

    /// El valor del ajuste: JSON, o vacío sin zonas.
    pub fn to_setting(zones: Option<&Self>) -> String {
        zones
            .and_then(|z| serde_json::to_string(z).ok())
            .unwrap_or_default()
    }
}

/// `#rrggbb` → `[r, g, b]`.
fn rgb(color: &str) -> Option<[u8; 3]> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

/// Canal sRGB (0–255) a lineal (0–1).
fn linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Luminancia relativa (WCAG 2).
fn luminance([r, g, b]: [u8; 3]) -> f64 {
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

/// Contraste WCAG 2 entre dos colores, de 1 a 21.
fn contrast(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Color en OKLab (https://bottosson.github.io/posts/oklab/).
fn oklab([r, g, b]: [u8; 3]) -> [f64; 3] {
    let (r, g, b) = (linear(r), linear(g), linear(b));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    ]
}

/// Distancia euclídea en OKLab, × 100.
fn distance(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (a, b) = (oklab(a), oklab(b));
    100.0 * ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zones(limits: &[f64], colors: &[&str]) -> Zones {
        Zones {
            limits: limits.to_vec(),
            colors: colors.iter().map(|c| (*c).to_string()).collect(),
        }
    }

    fn heart_rate() -> Zones {
        zones(
            &[120.0, 140.0, 160.0],
            &["#6b7280", "#2563eb", "#15803d", "#b91c1c"],
        )
    }

    #[test]
    fn a_value_on_a_limit_goes_up() {
        let z = heart_rate();
        assert_eq!(z.zone(0.0), 0);
        assert_eq!(z.zone(119.9), 0);
        assert_eq!(z.zone(120.0), 1);
        assert_eq!(z.zone(139.9), 1);
        assert_eq!(z.zone(140.0), 2);
        assert_eq!(z.zone(159.9), 2);
        assert_eq!(z.zone(160.0), 3);
        assert_eq!(z.zone(250.0), 3);
    }

    #[test]
    fn invalid_zones_say_why() {
        assert_eq!(heart_rate().problem(), None);
        let cases = [
            zones(&[], &["#6b7280"]),
            zones(&[120.0], &["#6b7280", "#2563eb", "#15803d"]),
            zones(&[140.0, 120.0], &["#6b7280", "#2563eb", "#15803d"]),
            zones(&[120.0, 120.0], &["#6b7280", "#2563eb", "#15803d"]),
            zones(&[0.0], &["#6b7280", "#2563eb"]),
            zones(&[f64::NAN], &["#6b7280", "#2563eb"]),
            zones(&[120.0], &["#6b7280", "azul"]),
            zones(&[120.0], &["#6b7280", "#2563e"]),
        ];
        for z in cases {
            assert!(z.problem().is_some(), "{z:?}");
        }
        let many = Zones {
            limits: (1..=MAX_ZONES).map(|v| v as f64).collect(),
            colors: vec!["#6b7280".into(); MAX_ZONES + 1],
        };
        assert!(many.problem().is_some());
    }

    #[test]
    fn contrast_and_distance_by_hand() {
        // Negro sobre blanco: 21 (el máximo de WCAG); un color consigo mismo, 1 y distancia 0.
        assert!((contrast([0, 0, 0], [255, 255, 255]) - 21.0).abs() < 1e-9);
        assert!((contrast([37, 99, 235], [37, 99, 235]) - 1.0).abs() < 1e-12);
        assert!(distance([37, 99, 235], [37, 99, 235]).abs() < 1e-12);
        // Del negro al blanco, la luminosidad de OKLab va de 0 a 1: distancia 100.
        assert!((distance([0, 0, 0], [255, 255, 255]) - 100.0).abs() < 0.01);
    }

    #[test]
    fn the_default_scale_and_the_suggested_colors_raise_no_warning() {
        // La escala por defecto del mapa (`--map-seq-*`) cumple los umbrales.
        let default = zones(
            &[1.0, 2.0, 3.0, 4.0],
            &["#6da7ec", "#3987e5", "#256abf", "#184f95", "#0d366b"],
        );
        assert!(default.warnings().is_empty(), "{:?}", default.warnings());
        // Y los colores que propone la interfaz (`ZONE_COLORS` en `api.ts`).
        let suggested = Zones {
            limits: (1..MAX_ZONES).map(|v| v as f64).collect(),
            colors: [
                "#6b7280", "#2563eb", "#15803d", "#d97706", "#b91c1c", "#7e22ce", "#0e7490",
                "#a16207", "#be185d", "#1e293b",
            ]
            .map(String::from)
            .to_vec(),
        };
        assert!(
            suggested.warnings().is_empty(),
            "{:?}",
            suggested.warnings()
        );
    }

    #[test]
    fn light_or_similar_colors_warn() {
        // Amarillo: casi no se ve sobre el fondo del mapa. Dos azules casi iguales, seguidos.
        let z = zones(&[120.0, 140.0], &["#ffeb3b", "#2563eb", "#2a65e8"]);
        assert_eq!(
            z.warnings(),
            vec![
                "La zona 1 se ve poco sobre el mapa: elige un color más oscuro o más intenso.",
                "Las zonas 2 y 3 tienen colores muy parecidos: cuesta distinguirlas.",
            ]
        );
    }

    #[test]
    fn settings_round_trip() {
        let z = heart_rate();
        let text = Zones::to_setting(Some(&z));
        assert_eq!(Zones::from_setting(Some(&text)), Some(z));
        assert_eq!(Zones::to_setting(None), "");
        assert_eq!(Zones::from_setting(Some("")), None);
        assert_eq!(Zones::from_setting(None), None);
        // Unas zonas guardadas que no valen se ignoran.
        let bad = r##"{"limits":[140,120],"colors":["#6b7280","#2563eb","#15803d"]}"##;
        assert_eq!(Zones::from_setting(Some(bad)), None);
    }
}
