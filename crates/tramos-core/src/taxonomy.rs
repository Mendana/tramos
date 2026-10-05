//! Taxonomía de errores y etiquetas del corredor sobre un tramo (`docs/taxonomia.md`).
//!
//! La taxonomía (tipos, subtipos y causas) vive en un fichero de datos versionado,
//! `data/taxonomy.json`, y no en el código: el código solo conoce sus claves a través de él.
//! Una etiqueta tiene tres niveles y ninguno es obligatorio: confirmar si hubo error, tipo y
//! subtipo, y contexto.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// La taxonomía con la que se compila la app.
pub const BUILTIN_TAXONOMY_JSON: &str = include_str!("../data/taxonomy.json");

/// Esfuerzo percibido: de 1 a 10.
pub const EFFORT_RANGE: std::ops::RangeInclusive<u8> = 1..=10;

/// Taxonomía de errores: tipos con sus subtipos y causas percibidas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Taxonomy {
    /// Versión del fichero. Cada etiqueta guarda la versión con la que se escribió.
    pub version: String,
    pub types: Vec<ErrorType>,
    pub causes: Vec<TaxonomyEntry>,
}

/// Tipo de error (nivel 2) con sus subtipos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorType {
    pub key: String,
    pub label: String,
    /// Aclaración para la interfaz, si hace falta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub subtypes: Vec<TaxonomyEntry>,
}

/// Subtipo o causa: clave estable y nombre para la interfaz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxonomyEntry {
    pub key: String,
    pub label: String,
}

/// Nivel 1: ¿hubo error en el tramo?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confirmation {
    Error,
    NoError,
    /// Se perdió tiempo, pero por el físico y no por orientarse mal.
    Physical,
}

/// Nivel 3: parte del tramo en la que se cometió el error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegPart {
    /// Al salir de la baliza.
    Start,
    Middle,
    /// Al atacar la baliza.
    Attack,
}

/// Etiqueta del corredor sobre un tramo. Todos los campos son opcionales.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LegTag {
    /// Nivel 1.
    #[serde(default)]
    pub confirmation: Option<Confirmation>,
    /// Nivel 2: clave de un tipo de la taxonomía.
    #[serde(default)]
    pub error_type: Option<String>,
    /// Nivel 2: clave de un subtipo de `error_type`.
    #[serde(default)]
    pub error_subtype: Option<String>,
    /// Nivel 3: claves de causas percibidas (varias).
    #[serde(default)]
    pub causes: Vec<String>,
    #[serde(default)]
    pub leg_part: Option<LegPart>,
    /// Segundos que el corredor cree haber perdido.
    #[serde(default)]
    pub perceived_loss_s: Option<f64>,
    /// Esfuerzo percibido, de 1 a 10.
    #[serde(default)]
    pub effort: Option<u8>,
    /// Nota libre.
    #[serde(default)]
    pub note: Option<String>,
}

impl LegTag {
    /// No dice nada: ningún nivel rellenado.
    pub fn is_empty(&self) -> bool {
        self.confirmation.is_none()
            && self.error_type.is_none()
            && self.error_subtype.is_none()
            && self.causes.is_empty()
            && self.leg_part.is_none()
            && self.perceived_loss_s.is_none()
            && self.effort.is_none()
            && self.note.is_none()
    }

    /// La misma etiqueta en forma canónica: nota sin espacios en los extremos (vacía = sin nota)
    /// y causas ordenadas y sin repetir.
    pub fn normalized(mut self) -> Self {
        self.note = self
            .note
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty());
        self.causes.sort();
        self.causes.dedup();
        self
    }
}

/// Errores de la taxonomía o de una etiqueta que no encaja en ella.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum TaxonomyError {
    #[error("el fichero de taxonomía no es válido: {0}")]
    Json(String),
    #[error("la taxonomía no tiene versión")]
    MissingVersion,
    #[error("la taxonomía tiene una clave o un nombre vacío")]
    EmptyEntry,
    #[error("la clave «{0}» está repetida en la taxonomía")]
    DuplicateKey(String),
    #[error("el tipo de error «{0}» no existe en la taxonomía")]
    UnknownType(String),
    #[error("el subtipo «{subtype}» no existe en el tipo «{error_type}»")]
    UnknownSubtype { error_type: String, subtype: String },
    #[error("hay subtipo sin tipo de error")]
    SubtypeWithoutType,
    #[error("la causa «{0}» no existe en la taxonomía")]
    UnknownCause(String),
    #[error("la causa «{0}» está repetida")]
    DuplicateCause(String),
    #[error("el esfuerzo tiene que ir de 1 a 10 (es {0})")]
    EffortOutOfRange(u8),
    #[error("los segundos perdidos tienen que ser un número positivo o cero (son {0})")]
    InvalidPerceivedLoss(f64),
}

impl Taxonomy {
    /// Lee y comprueba una taxonomía en JSON: versión, claves y nombres no vacíos, y claves sin
    /// repetir (los tipos y las causas entre sí, y los subtipos dentro de su tipo).
    pub fn parse(json: &str) -> Result<Self, TaxonomyError> {
        let taxonomy: Self =
            serde_json::from_str(json).map_err(|e| TaxonomyError::Json(e.to_string()))?;
        if taxonomy.version.trim().is_empty() {
            return Err(TaxonomyError::MissingVersion);
        }
        check_entries(
            taxonomy
                .types
                .iter()
                .map(|t| (t.key.as_str(), t.label.as_str())),
        )?;
        for t in &taxonomy.types {
            check_entries(
                t.subtypes
                    .iter()
                    .map(|s| (s.key.as_str(), s.label.as_str())),
            )?;
        }
        check_entries(
            taxonomy
                .causes
                .iter()
                .map(|c| (c.key.as_str(), c.label.as_str())),
        )?;
        Ok(taxonomy)
    }

    /// La taxonomía de `data/taxonomy.json`.
    pub fn builtin() -> Result<Self, TaxonomyError> {
        Self::parse(BUILTIN_TAXONOMY_JSON)
    }

    /// El tipo de error con esa clave.
    pub fn error_type(&self, key: &str) -> Option<&ErrorType> {
        self.types.iter().find(|t| t.key == key)
    }

    /// Comprueba que una etiqueta encaja en la taxonomía: tipo, subtipo (del mismo tipo) y causas
    /// existentes, causas sin repetir, esfuerzo de 1 a 10 y segundos perdidos finitos y no
    /// negativos.
    pub fn validate(&self, tag: &LegTag) -> Result<(), TaxonomyError> {
        match (&tag.error_type, &tag.error_subtype) {
            (None, Some(_)) => return Err(TaxonomyError::SubtypeWithoutType),
            (None, None) => {}
            (Some(key), subtype) => {
                let error_type = self
                    .error_type(key)
                    .ok_or_else(|| TaxonomyError::UnknownType(key.clone()))?;
                if let Some(subtype) = subtype {
                    if !error_type.subtypes.iter().any(|s| &s.key == subtype) {
                        return Err(TaxonomyError::UnknownSubtype {
                            error_type: key.clone(),
                            subtype: subtype.clone(),
                        });
                    }
                }
            }
        }
        for (i, cause) in tag.causes.iter().enumerate() {
            if !self.causes.iter().any(|c| &c.key == cause) {
                return Err(TaxonomyError::UnknownCause(cause.clone()));
            }
            if tag.causes[..i].contains(cause) {
                return Err(TaxonomyError::DuplicateCause(cause.clone()));
            }
        }
        if let Some(effort) = tag.effort {
            if !EFFORT_RANGE.contains(&effort) {
                return Err(TaxonomyError::EffortOutOfRange(effort));
            }
        }
        if let Some(s) = tag.perceived_loss_s {
            if !s.is_finite() || s < 0.0 {
                return Err(TaxonomyError::InvalidPerceivedLoss(s));
            }
        }
        Ok(())
    }
}

/// Claves y nombres no vacíos y claves sin repetir.
fn check_entries<'a>(
    entries: impl Iterator<Item = (&'a str, &'a str)>,
) -> Result<(), TaxonomyError> {
    let mut seen: Vec<&str> = Vec::new();
    for (key, label) in entries {
        if key.trim().is_empty() || label.trim().is_empty() {
            return Err(TaxonomyError::EmptyEntry);
        }
        if seen.contains(&key) {
            return Err(TaxonomyError::DuplicateKey(key.to_string()));
        }
        seen.push(key);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn taxonomy() -> Taxonomy {
        Taxonomy::builtin().expect("la taxonomía incluida es válida")
    }

    #[test]
    fn builtin_taxonomy_matches_the_document() {
        let t = taxonomy();
        assert_eq!(t.version, "0");
        let types: Vec<_> = t.types.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(
            types,
            [
                "Salida de baliza",
                "Elección de ruta",
                "Navegación",
                "Ataque",
                "Físico",
                "Otro"
            ]
        );
        let subtypes: usize = t.types.iter().map(|t| t.subtypes.len()).sum();
        assert_eq!(subtypes, 18);
        assert_eq!(t.causes.len(), 6);
    }

    #[test]
    fn parse_rejects_broken_taxonomies() {
        let ok = r#"{"version":"1","types":[{"key":"a","label":"A","subtypes":[{"key":"x","label":"X"}]}],"causes":[]}"#;
        assert!(Taxonomy::parse(ok).is_ok());
        assert_eq!(
            Taxonomy::parse(&ok.replace(r#""version":"1""#, r#""version":" ""#)),
            Err(TaxonomyError::MissingVersion)
        );
        assert_eq!(
            Taxonomy::parse(&ok.replace(r#""label":"X""#, r#""label":"""#)),
            Err(TaxonomyError::EmptyEntry)
        );
        let repeated_subtype = ok.replace(
            r#"[{"key":"x","label":"X"}]"#,
            r#"[{"key":"x","label":"X"},{"key":"x","label":"Y"}]"#,
        );
        assert_eq!(
            Taxonomy::parse(&repeated_subtype),
            Err(TaxonomyError::DuplicateKey("x".into()))
        );
        let repeated_cause = ok.replace(
            r#""causes":[]"#,
            r#""causes":[{"key":"c","label":"C"},{"key":"c","label":"D"}]"#,
        );
        assert_eq!(
            Taxonomy::parse(&repeated_cause),
            Err(TaxonomyError::DuplicateKey("c".into()))
        );
        assert!(matches!(Taxonomy::parse("{}"), Err(TaxonomyError::Json(_))));
    }

    #[test]
    fn the_same_subtype_key_can_appear_in_two_types() {
        let json = r#"{"version":"1","types":[
            {"key":"a","label":"A","subtypes":[{"key":"x","label":"X"}]},
            {"key":"b","label":"B","subtypes":[{"key":"x","label":"X"}]}],"causes":[]}"#;
        assert!(Taxonomy::parse(json).is_ok());
    }

    #[test]
    fn validate_accepts_any_combination_of_levels() {
        let t = taxonomy();
        assert_eq!(t.validate(&LegTag::default()), Ok(()));
        let full = LegTag {
            confirmation: Some(Confirmation::Error),
            error_type: Some("navigation".into()),
            error_subtype: Some("parallel".into()),
            causes: vec!["rush".into(), "fatigue".into()],
            leg_part: Some(LegPart::Attack),
            perceived_loss_s: Some(45.0),
            effort: Some(10),
            note: Some("Bajé por la vaguada equivocada".into()),
        };
        assert_eq!(t.validate(&full), Ok(()));
        // Solo el tipo, sin confirmar ni subtipo.
        let only_type = LegTag {
            error_type: Some("attack".into()),
            ..LegTag::default()
        };
        assert_eq!(t.validate(&only_type), Ok(()));
    }

    #[test]
    fn validate_rejects_what_is_not_in_the_taxonomy() {
        let t = taxonomy();
        let tag = |f: fn(&mut LegTag)| {
            let mut tag = LegTag::default();
            f(&mut tag);
            t.validate(&tag)
        };
        assert_eq!(
            tag(|t| t.error_type = Some("magic".into())),
            Err(TaxonomyError::UnknownType("magic".into()))
        );
        // Subtipo de otro tipo.
        assert_eq!(
            tag(|t| {
                t.error_type = Some("navigation".into());
                t.error_subtype = Some("fall".into());
            }),
            Err(TaxonomyError::UnknownSubtype {
                error_type: "navigation".into(),
                subtype: "fall".into()
            })
        );
        assert_eq!(
            tag(|t| t.error_subtype = Some("parallel".into())),
            Err(TaxonomyError::SubtypeWithoutType)
        );
        assert_eq!(
            tag(|t| t.causes = vec!["luck".into()]),
            Err(TaxonomyError::UnknownCause("luck".into()))
        );
        assert_eq!(
            tag(|t| t.causes = vec!["rush".into(), "rush".into()]),
            Err(TaxonomyError::DuplicateCause("rush".into()))
        );
        assert_eq!(
            tag(|t| t.effort = Some(0)),
            Err(TaxonomyError::EffortOutOfRange(0))
        );
        assert_eq!(
            tag(|t| t.effort = Some(11)),
            Err(TaxonomyError::EffortOutOfRange(11))
        );
        assert_eq!(
            tag(|t| t.perceived_loss_s = Some(-1.0)),
            Err(TaxonomyError::InvalidPerceivedLoss(-1.0))
        );
        assert!(matches!(
            tag(|t| t.perceived_loss_s = Some(f64::NAN)),
            Err(TaxonomyError::InvalidPerceivedLoss(_))
        ));
    }

    #[test]
    fn normalized_trims_the_note_and_sorts_the_causes() {
        let tag = LegTag {
            causes: vec!["rush".into(), "fatigue".into(), "rush".into()],
            note: Some("  ".into()),
            ..LegTag::default()
        }
        .normalized();
        assert_eq!(tag.causes, ["fatigue", "rush"]);
        assert_eq!(tag.note, None);
        assert!(!tag.is_empty());
        assert!(
            LegTag {
                note: Some(" ".into()),
                ..LegTag::default()
            }
            .normalized()
            .is_empty()
        );
        assert_eq!(
            LegTag {
                note: Some(" vale ".into()),
                ..LegTag::default()
            }
            .normalized()
            .note
            .as_deref(),
            Some("vale")
        );
    }

    #[test]
    fn tags_serialize_with_snake_case_values() {
        let tag = LegTag {
            confirmation: Some(Confirmation::NoError),
            leg_part: Some(LegPart::Start),
            ..LegTag::default()
        };
        let json = serde_json::to_value(&tag).expect("se serializa");
        assert_eq!(json["confirmation"], "no_error");
        assert_eq!(json["leg_part"], "start");
        // Lo que no llega se queda sin rellenar.
        let parsed: LegTag =
            serde_json::from_str(r#"{"confirmation":"physical"}"#).expect("se lee");
        assert_eq!(parsed.confirmation, Some(Confirmation::Physical));
        assert!(parsed.causes.is_empty());
    }
}
