//! Tipo de una carrera importada: competición o entrenamiento (#195).
//!
//! Hay corredores que suben sus entrenamientos con mapa además de las carreras. Se analizan
//! igual, pero no tienen por qué comportarse igual, así que el histórico deja filtrarlos
//! (`docs/historico.md`, "Filtro"). No viene en el .spl ni en OReplay: lo marca el corredor.

use serde::{Deserialize, Serialize};

/// Competición o entrenamiento. Por defecto, competición: es lo que era todo antes de #195.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    #[default]
    Race,
    Training,
}

impl EventKind {
    /// Para `skip_serializing_if`: una competición no hace falta escribirla.
    pub fn is_race(&self) -> bool {
        *self == Self::Race
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_in_snake_case_and_defaults_to_race() {
        assert_eq!(serde_json::to_string(&EventKind::Race).unwrap(), "\"race\"");
        assert_eq!(
            serde_json::from_str::<EventKind>("\"training\"").unwrap(),
            EventKind::Training
        );
        assert_eq!(EventKind::default(), EventKind::Race);
    }
}
