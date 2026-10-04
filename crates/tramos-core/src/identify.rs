//! Identificar al usuario dentro de una carrera.
//!
//! Dada la identidad que el usuario configura en los ajustes ([`RunnerIdentity`]: tarjeta SI
//! y nombre completo, ambos opcionales), [`identify_runner`] busca sus resultados en un
//! [`Event`] y devuelve una coincidencia única, varios candidatos o ninguno, para que la
//! interfaz pregunte cuando haga falta. Las reglas están en `docs/identificacion.md`:
//!
//! - Primero la tarjeta SI; si no hay tarjeta configurada o no casa con nadie, el nombre.
//! - Los nombres se comparan normalizados ([`normalize_name`]): sin acentos, sin distinguir
//!   mayúsculas, con los espacios y signos colapsados. El .spl trae nombre y apellidos por
//!   separado; casan en el orden «nombre apellidos» o «apellidos nombre».
//! - Si la tarjeta casa pero el nombre configurado no, la coincidencia se marca para que la
//!   interfaz pida confirmación (tarjeta prestada o reutilizada); si además el nombre casa con
//!   otro resultado, se devuelven los dos como candidatos.
//!
//! Los resultados se señalan por su posición en el `Event` ([`ResultRef`]), nunca por
//! `Runner::id`, que no es único (`docs/formato-spl.md`).

use serde::{Deserialize, Serialize};

use crate::model::{Event, RaceResult, Runner};

/// Identidad del usuario tal y como la configura en los ajustes.
///
/// En JSON: `{"si_card": 2001234, "full_name": "Ana Pérez García"}`; los dos campos pueden ser
/// `null` o faltar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RunnerIdentity {
    /// Número de tarjeta SportIdent propia.
    pub si_card: Option<u32>,
    /// Nombre y apellidos, en el orden y con la grafía que el usuario quiera:
    /// `"Nombre Apellido1 Apellido2"`, `"Apellido1 Apellido2, Nombre"`…
    pub full_name: Option<String>,
}

/// Posición de un resultado en un [`Event`]: categoría y resultado dentro de ella.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResultRef {
    /// Posición de la categoría en `Event::classes`.
    pub class_index: usize,
    /// Posición del resultado en `Class::results`.
    pub result_index: usize,
}

impl ResultRef {
    /// El resultado al que apunta, si `event` es la carrera de la que salió.
    pub fn get<'a>(&self, event: &'a Event) -> Option<&'a RaceResult> {
        event
            .classes
            .get(self.class_index)?
            .results
            .get(self.result_index)
    }
}

/// Resultado que puede ser del usuario y por qué.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub result: ResultRef,
    /// Su tarjeta SI es la configurada.
    pub si_card_matches: bool,
    /// Su nombre casa con el configurado.
    pub name_matches: bool,
}

/// Qué resultado de la carrera es del usuario.
///
/// En JSON lleva el tipo en `kind`: `{"kind": "unique", "candidate": {…}, "name_mismatch":
/// false}`, `{"kind": "ambiguous", "candidates": […]}` o `{"kind": "not_found"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Identification {
    /// Un solo resultado casa.
    Unique {
        candidate: Candidate,
        /// Casa por tarjeta, pero el nombre configurado no coincide con el del resultado: la
        /// tarjeta puede ser prestada o reutilizada. La interfaz debería pedir confirmación.
        name_mismatch: bool,
    },
    /// Varios resultados casan, en el orden de la carrera: la interfaz pregunta.
    Ambiguous { candidates: Vec<Candidate> },
    /// Ninguno casa (o no hay identidad configurada): la interfaz deja elegir de la lista.
    NotFound,
}

/// Busca al usuario en `event` (ver el módulo y `docs/identificacion.md`).
pub fn identify_runner(event: &Event, identity: &RunnerIdentity) -> Identification {
    let name = identity
        .full_name
        .as_deref()
        .map(normalize_name)
        .filter(|name| !name.is_empty());

    let candidates: Vec<Candidate> = results(event)
        .map(|(result, runner)| Candidate {
            result,
            si_card_matches: identity.si_card.is_some() && runner.si_card == identity.si_card,
            name_matches: name
                .as_deref()
                .is_some_and(|name| name_matches(runner, name)),
        })
        .filter(|c| c.si_card_matches || c.name_matches)
        .collect();

    let by_card = || candidates.iter().filter(|c| c.si_card_matches);
    let chosen: Vec<Candidate> = if by_card().next().is_none() {
        // Sin tarjeta o sin nadie con ella: solo cuenta el nombre.
        candidates
    } else if name.is_none() {
        by_card().copied().collect()
    } else if by_card().any(|c| c.name_matches) {
        // La tarjeta y el nombre señalan al mismo resultado: los que solo casan por una de las
        // dos cosas se descartan.
        by_card().filter(|c| c.name_matches).copied().collect()
    } else {
        // La tarjeta casa con alguien de otro nombre: puede estar prestada. Se ofrecen los de
        // la tarjeta y los del nombre; si solo hay uno, sale como único con aviso.
        candidates
    };

    match chosen.as_slice() {
        [] => Identification::NotFound,
        [candidate] => Identification::Unique {
            candidate: *candidate,
            name_mismatch: name.is_some() && !candidate.name_matches,
        },
        _ => Identification::Ambiguous { candidates: chosen },
    }
}

/// Normaliza un nombre para compararlo:
///
/// - Minúsculas y sin diacríticos: `á` → `a`, `ü` → `u`, `ñ` → `n`, `ç` → `c`, y el resto de
///   letras de Latin-1 (`æ` → `ae`, `ß` → `ss`…). Las marcas combinantes (texto en NFD) se
///   quitan. Fuera de Latin-1, que es lo único que puede traer un .spl, las letras se quedan
///   como están, en minúscula.
/// - Todo lo que no es letra ni dígito (espacios, guiones, apóstrofos, comas, puntos, `_`…)
///   separa palabras, y las palabras quedan separadas por un solo espacio, sin espacios al
///   principio ni al final.
///
/// `"  JOSÉ-Luis   Muñoz "` → `"jose luis munoz"`.
pub fn normalize_name(name: &str) -> String {
    let mut folded = String::with_capacity(name.len());
    for c in name.chars().flat_map(char::to_lowercase) {
        match fold_latin1(c) {
            Some(ascii) => folded.push_str(ascii),
            None if c.is_alphanumeric() => folded.push(c),
            None => folded.push(' '),
        }
    }
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Letra minúscula de Latin-1 sin su diacrítico; `""` para las marcas combinantes.
fn fold_latin1(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{300}'..='\u{36f}' => "",
        'à'..='å' | 'ª' => "a",
        'æ' => "ae",
        'ç' => "c",
        'è'..='ë' => "e",
        'ì'..='ï' => "i",
        'ð' => "d",
        'ñ' => "n",
        'ò'..='ö' | 'ø' | 'º' => "o",
        'ù'..='ü' => "u",
        'ý' | 'ÿ' => "y",
        'þ' => "th",
        'ß' => "ss",
        _ => return None,
    })
}

/// El nombre del corredor, en el orden «nombre apellidos» o «apellidos nombre», es `name`
/// (ya normalizado y no vacío).
fn name_matches(runner: &Runner, name: &str) -> bool {
    let given = &runner.given_name;
    let family = &runner.family_name;
    normalize_name(&format!("{given} {family}")) == name
        || normalize_name(&format!("{family} {given}")) == name
}

fn results(event: &Event) -> impl Iterator<Item = (ResultRef, &Runner)> {
    event
        .classes
        .iter()
        .enumerate()
        .flat_map(|(class_index, class)| {
            class
                .results
                .iter()
                .enumerate()
                .map(move |(result_index, result)| {
                    (
                        ResultRef {
                            class_index,
                            result_index,
                        },
                        &result.runner,
                    )
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Class, Course, RaceStatus};
    use chrono::NaiveDate;
    use serde_json::json;

    /// Todos los nombres son inventados.
    fn result(given: &str, family: &str, si_card: Option<u32>) -> RaceResult {
        RaceResult {
            runner: Runner {
                // Mismo valor para todos, como pasa con el `0x80` del .spl: no se usa.
                id: 150,
                given_name: given.into(),
                family_name: family.into(),
                club: None,
                bib: None,
                si_card,
                sex: None,
            },
            status: RaceStatus::Ok,
            place: None,
            punches: Vec::new(),
        }
    }

    fn class(name: &str, results: Vec<RaceResult>) -> Class {
        Class {
            id: 1,
            name: name.into(),
            short_name: None,
            course: Course {
                controls: vec![31, 45],
            },
            results,
        }
    }

    /// Dos categorías; «Lucía Ibáñez Roldán» está en las dos (dos personas homónimas).
    fn event() -> Event {
        Event {
            name: None,
            date: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
            classes: vec![
                class(
                    "M21",
                    vec![
                        result("José Ángel", "Muñoz Castaño", Some(2_001_001)),
                        result("Íñigo", "de la Peña-Gómez", Some(2_001_002)),
                        result("Tomás", "Ruiz", None),
                    ],
                ),
                class(
                    "F21",
                    vec![
                        result("Lucía", "Ibáñez Roldán", Some(2_001_003)),
                        result("Begoña", "Sáez", Some(2_001_004)),
                    ],
                ),
                class(
                    "F35",
                    vec![
                        result("Marta", "Ortiz", Some(2_001_005)),
                        result("LUCÍA", "IBÁÑEZ ROLDÁN", None),
                    ],
                ),
            ],
        }
    }

    fn at(class_index: usize, result_index: usize) -> ResultRef {
        ResultRef {
            class_index,
            result_index,
        }
    }

    fn by_name(name: &str) -> Identification {
        identify_runner(
            &event(),
            &RunnerIdentity {
                si_card: None,
                full_name: Some(name.into()),
            },
        )
    }

    fn unique(result: ResultRef, si_card: bool, name: bool, name_mismatch: bool) -> Identification {
        Identification::Unique {
            candidate: Candidate {
                result,
                si_card_matches: si_card,
                name_matches: name,
            },
            name_mismatch,
        }
    }

    fn unique_by_name(result: ResultRef) -> Identification {
        unique(result, false, true, false)
    }

    #[test]
    fn normalize_removes_accents_case_and_extra_spaces() {
        assert_eq!(normalize_name("  JOSÉ-Luis   Muñoz "), "jose luis munoz");
        assert_eq!(normalize_name("Íñigo de la PEÑA"), "inigo de la pena");
        assert_eq!(normalize_name("ÑANDÚ Çelik Øster"), "nandu celik oster");
        assert_eq!(normalize_name("Gómez, Ana\tMaría"), "gomez ana maria");
        assert_eq!(normalize_name("D'Ávila"), "d avila");
        assert_eq!(normalize_name("D-Ávila"), "d avila");
        assert_eq!(normalize_name("DÁvila"), "davila");
        assert_eq!(normalize_name("N1__ Apellido1_____"), "n1 apellido1");
        assert_eq!(normalize_name("Straße Æsir"), "strasse aesir");
        // NFD: «é» como «e» + acento combinante.
        assert_eq!(normalize_name("Jose\u{301} Mun\u{303}oz"), "jose munoz");
        assert_eq!(normalize_name("   "), "");
    }

    #[test]
    fn name_matches_with_or_without_accents() {
        let jose = at(0, 0);
        assert_eq!(by_name("José Ángel Muñoz Castaño"), unique_by_name(jose));
        assert_eq!(by_name("Jose Angel Munoz Castano"), unique_by_name(jose));
        assert_eq!(by_name("jose angel MUNOZ castaño"), unique_by_name(jose));
        assert_eq!(by_name("Begona Saez"), unique_by_name(at(1, 1)));
        assert_eq!(by_name("BEGOÑA SÁEZ"), unique_by_name(at(1, 1)));
    }

    #[test]
    fn name_ignores_case_and_repeated_spaces() {
        assert_eq!(by_name("  TOMÁS    ruiz  "), unique_by_name(at(0, 2)));
        assert_eq!(by_name("tomas\truiz"), unique_by_name(at(0, 2)));
    }

    #[test]
    fn compound_names_and_surnames() {
        let inigo = unique_by_name(at(0, 1));
        // Apellido compuesto con partícula y guion: el guion es como un espacio.
        assert_eq!(by_name("Íñigo de la Peña-Gómez"), inigo);
        assert_eq!(by_name("Inigo De La Pena Gomez"), inigo);
        // También «apellidos, nombre».
        assert_eq!(by_name("de la Peña Gómez, Íñigo"), inigo);
        assert_eq!(
            by_name("Muñoz Castaño José Ángel"),
            unique_by_name(at(0, 0))
        );
        // Lo que no casa: nombre incompleto, apellidos sin la partícula o en otro orden.
        assert_eq!(by_name("José Muñoz Castaño"), Identification::NotFound);
        assert_eq!(by_name("José Ángel Muñoz"), Identification::NotFound);
        assert_eq!(by_name("Íñigo Peña Gómez"), Identification::NotFound);
        assert_eq!(by_name("Íñigo Gómez de la Peña"), Identification::NotFound);
    }

    #[test]
    fn split_between_given_and_family_name_does_not_matter() {
        let identity = RunnerIdentity {
            si_card: None,
            full_name: Some("Maria Jose Ruiz".into()),
        };
        let mut event = event();
        let splits = [
            ("María José", "Ruiz"),
            ("María", "José Ruiz"),
            ("María José Ruiz", ""),
            ("", "María José Ruiz"),
        ];
        for (given, family) in splits {
            let runner = &mut event.classes[0].results[2].runner;
            runner.given_name = given.into();
            runner.family_name = family.into();
            assert_eq!(
                identify_runner(&event, &identity),
                unique_by_name(at(0, 2)),
                "{given:?} / {family:?}"
            );
        }
    }

    #[test]
    fn same_name_in_two_classes_gives_both_candidates() {
        let candidate = |result| Candidate {
            result,
            si_card_matches: false,
            name_matches: true,
        };
        assert_eq!(
            by_name("Lucia Ibanez Roldan"),
            Identification::Ambiguous {
                candidates: vec![candidate(at(1, 0)), candidate(at(2, 1))],
            }
        );
    }

    #[test]
    fn card_match_is_unique() {
        let identity = RunnerIdentity {
            si_card: Some(2_001_003),
            full_name: None,
        };
        assert_eq!(
            identify_runner(&event(), &identity),
            unique(at(1, 0), true, false, false)
        );
    }

    #[test]
    fn card_disambiguates_namesakes() {
        let identity = RunnerIdentity {
            si_card: Some(2_001_003),
            full_name: Some("Lucía Ibáñez Roldán".into()),
        };
        assert_eq!(
            identify_runner(&event(), &identity),
            unique(at(1, 0), true, true, false)
        );
    }

    #[test]
    fn card_not_in_the_event_falls_back_to_the_name() {
        let identity = RunnerIdentity {
            si_card: Some(9_999_999),
            full_name: Some("tomas ruiz".into()),
        };
        assert_eq!(
            identify_runner(&event(), &identity),
            unique_by_name(at(0, 2))
        );
    }

    #[test]
    fn card_with_another_name_is_flagged() {
        // La tarjeta de Marta Ortiz, pero el usuario se llama de otra forma y no aparece:
        // tarjeta prestada o alquilada. Única, con aviso.
        let identity = RunnerIdentity {
            si_card: Some(2_001_005),
            full_name: Some("Ana Quintana".into()),
        };
        assert_eq!(
            identify_runner(&event(), &identity),
            unique(at(2, 0), true, false, true)
        );
    }

    #[test]
    fn card_lent_to_someone_else_gives_both_candidates() {
        // El usuario (Tomás Ruiz, sin tarjeta en el fichero) prestó la suya a Begoña Sáez.
        let identity = RunnerIdentity {
            si_card: Some(2_001_004),
            full_name: Some("Tomás Ruiz".into()),
        };
        assert_eq!(
            identify_runner(&event(), &identity),
            Identification::Ambiguous {
                candidates: vec![
                    Candidate {
                        result: at(0, 2),
                        si_card_matches: false,
                        name_matches: true,
                    },
                    Candidate {
                        result: at(1, 1),
                        si_card_matches: true,
                        name_matches: false,
                    },
                ],
            }
        );
    }

    #[test]
    fn same_card_twice_is_resolved_by_the_name() {
        let mut event = event();
        // La misma tarjeta en dos resultados (p. ej. prestada en otra categoría).
        event.classes[2].results[0].runner.si_card = Some(2_001_001);
        let card_only = RunnerIdentity {
            si_card: Some(2_001_001),
            full_name: None,
        };
        assert!(matches!(
            identify_runner(&event, &card_only),
            Identification::Ambiguous { candidates } if candidates.len() == 2
        ));
        let with_name = RunnerIdentity {
            full_name: Some("José Ángel Muñoz Castaño".into()),
            ..card_only
        };
        assert_eq!(
            identify_runner(&event, &with_name),
            unique(at(0, 0), true, true, false)
        );
    }

    #[test]
    fn nobody_matches() {
        assert_eq!(by_name("Ana Quintana"), Identification::NotFound);
        let identity = RunnerIdentity {
            si_card: Some(9_999_999),
            full_name: Some("Ana Quintana".into()),
        };
        assert_eq!(
            identify_runner(&event(), &identity),
            Identification::NotFound
        );
    }

    #[test]
    fn empty_identity_matches_nobody() {
        assert_eq!(
            identify_runner(&event(), &RunnerIdentity::default()),
            Identification::NotFound
        );
        assert_eq!(by_name("  - "), Identification::NotFound);

        // Un corredor sin nombre en el fichero no casa con un nombre en blanco.
        let mut event = event();
        event.classes[0].results[2].runner.given_name.clear();
        event.classes[0].results[2].runner.family_name.clear();
        let blank = RunnerIdentity {
            si_card: None,
            full_name: Some(" ".into()),
        };
        assert_eq!(identify_runner(&event, &blank), Identification::NotFound);
    }

    #[test]
    fn result_ref_points_into_the_event() {
        let event = event();
        let runner = &at(1, 1).get(&event).unwrap().runner;
        assert_eq!(runner.given_name, "Begoña");
        assert_eq!(at(1, 2).get(&event), None);
        assert_eq!(at(3, 0).get(&event), None);
    }

    #[test]
    fn json_shapes() {
        let identity: RunnerIdentity =
            serde_json::from_value(json!({"full_name": "Tomás Ruiz"})).unwrap();
        assert_eq!(identity.si_card, None);
        assert_eq!(
            serde_json::to_value(&identity).unwrap(),
            json!({"si_card": null, "full_name": "Tomás Ruiz"})
        );

        assert_eq!(
            serde_json::to_value(unique(at(0, 2), true, false, true)).unwrap(),
            json!({
                "kind": "unique",
                "candidate": {
                    "result": {"class_index": 0, "result_index": 2},
                    "si_card_matches": true,
                    "name_matches": false
                },
                "name_mismatch": true
            })
        );
        let ambiguous = by_name("lucia ibanez roldan");
        let value = serde_json::to_value(&ambiguous).unwrap();
        assert_eq!(value["kind"], json!("ambiguous"));
        assert_eq!(value["candidates"].as_array().unwrap().len(), 2);
        assert_eq!(
            serde_json::from_value::<Identification>(value).unwrap(),
            ambiguous
        );
        assert_eq!(
            serde_json::to_value(Identification::NotFound).unwrap(),
            json!({"kind": "not_found"})
        );
    }
}
