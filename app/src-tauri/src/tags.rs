//! Etiquetado de errores (`docs/taxonomia.md`): la taxonomía y las etiquetas de los tramos de
//! un resultado. La taxonomía es la de `tramos_core::taxonomy`; aquí solo se comprueba que una
//! etiqueta encaja en ella y en el recorrido antes de guardarla.

use chrono::{DateTime, Utc};
use serde::Serialize;
use thiserror::Error;
use tramos_core::taxonomy::{LegTag, Taxonomy, TaxonomyError};
use tramos_store::{ResultId, Store, StoreError, StoredTag};

/// Errores al etiquetar. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum TagError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Taxonomy(#[from] TaxonomyError),
    #[error("el tramo {leg} no existe: el recorrido tiene {legs}")]
    LegOutOfRange { leg: usize, legs: usize },
}

/// Etiqueta de un tramo, tal y como la guarda la base de datos.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TagView {
    pub leg_index: usize,
    pub taxonomy_version: String,
    pub tag: LegTag,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<StoredTag> for TagView {
    fn from(t: StoredTag) -> Self {
        Self {
            leg_index: t.leg_index,
            taxonomy_version: t.taxonomy_version,
            tag: t.tag,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}

/// La taxonomía con la que se etiqueta.
pub fn taxonomy() -> Result<Taxonomy, TagError> {
    Ok(Taxonomy::builtin()?)
}

/// Etiquetas de los tramos de un resultado, por tramo.
pub fn leg_tags(store: &Store, result_id: i64) -> Result<Vec<TagView>, TagError> {
    Ok(store
        .tags(ResultId(result_id))?
        .into_iter()
        .map(TagView::from)
        .collect())
}

/// Guarda la etiqueta del tramo `leg_index` (desde 1) de un resultado, en forma canónica
/// ([`LegTag::normalized`]) y con la versión de la taxonomía. Comprueba antes que el tramo
/// existe en el recorrido y que la etiqueta encaja en la taxonomía. Una etiqueta vacía borra la
/// del tramo y devuelve `None`.
pub fn save_leg_tag(
    store: &mut Store,
    result_id: i64,
    leg_index: usize,
    tag: LegTag,
) -> Result<Option<TagView>, TagError> {
    let result = ResultId(result_id);
    let legs = leg_count(store, result)?;
    if leg_index == 0 || leg_index > legs {
        return Err(TagError::LegOutOfRange {
            leg: leg_index,
            legs,
        });
    }
    let taxonomy = Taxonomy::builtin()?;
    let tag = tag.normalized();
    taxonomy.validate(&tag)?;
    Ok(store
        .save_tag(result, leg_index, &taxonomy.version, &tag)?
        .map(TagView::from))
}

/// Tramos del recorrido de un resultado: uno más que balizas (de la salida a la meta).
fn leg_count(store: &Store, result: ResultId) -> Result<usize, TagError> {
    let (event_id, at) = store.result_ref(result)?;
    let event = store.load_event(event_id)?;
    let class = event
        .classes
        .get(at.class_index)
        .ok_or(StoreError::ResultNotFound(result.0))?;
    Ok(class.course.controls.len() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::races::{race_detail, tests::imported};
    use tramos_core::taxonomy::{Confirmation, LegPart};

    #[test]
    fn tags_are_validated_normalized_and_saved_with_the_taxonomy_version() {
        let (mut store, result_id) = imported();
        let tag = LegTag {
            confirmation: Some(Confirmation::Error),
            error_type: Some("attack".into()),
            error_subtype: Some("confusing_area".into()),
            causes: vec!["rush".into(), "fatigue".into()],
            leg_part: Some(LegPart::Attack),
            note: Some("  ".into()),
            ..LegTag::default()
        };
        let saved = save_leg_tag(&mut store, result_id, 4, tag)
            .unwrap()
            .unwrap();
        assert_eq!(saved.taxonomy_version, taxonomy().unwrap().version);
        assert_eq!(saved.tag.causes, ["fatigue", "rush"]);
        assert_eq!(saved.tag.note, None);
        assert_eq!(leg_tags(&store, result_id).unwrap(), [saved]);

        // Vacía: se borra.
        assert_eq!(
            save_leg_tag(&mut store, result_id, 4, LegTag::default()).unwrap(),
            None
        );
        assert!(leg_tags(&store, result_id).unwrap().is_empty());
    }

    #[test]
    fn every_leg_of_the_course_can_be_tagged_and_no_other() {
        let (mut store, result_id) = imported();
        let legs = race_detail(&store, result_id)
            .unwrap()
            .report
            .lost_time
            .legs
            .len();
        let confirm = LegTag {
            confirmation: Some(Confirmation::NoError),
            ..LegTag::default()
        };
        // El último tramo (a meta) también.
        assert!(save_leg_tag(&mut store, result_id, legs, confirm.clone()).is_ok());
        for leg in [0, legs + 1] {
            assert!(matches!(
                save_leg_tag(&mut store, result_id, leg, confirm.clone()),
                Err(TagError::LegOutOfRange { leg: l, legs: n }) if l == leg && n == legs
            ));
        }
    }

    #[test]
    fn tags_outside_the_taxonomy_are_rejected_and_not_saved() {
        let (mut store, result_id) = imported();
        let tag = LegTag {
            error_type: Some("navigation".into()),
            error_subtype: Some("fall".into()),
            ..LegTag::default()
        };
        assert!(matches!(
            save_leg_tag(&mut store, result_id, 2, tag),
            Err(TagError::Taxonomy(TaxonomyError::UnknownSubtype { .. }))
        ));
        assert!(leg_tags(&store, result_id).unwrap().is_empty());
    }
}
