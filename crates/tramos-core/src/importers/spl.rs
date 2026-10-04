//! Lector de ficheros .spl de WinSplits (cabecera `spl4`).
//!
//! Porta el lector de referencia `tools/reference/winsplits_spl.py`; el formato y las
//! decisiones de importación están en `docs/formato-spl.md`. Resumen:
//!
//! - La cabecera se recorre por etiquetas hasta su marcador de fin (`0x2c`): de ella salen el
//!   nombre de la prueba (`0x14`), la fecha (`0x19`) y el número de categorías (`0x1f`), que se
//!   comprueba con las categorías leídas. Los registros de categoría empiezan justo después.
//! - Las horas de las picadas son centésimas desde la medianoche local de la carrera
//!   ([`RACE_TIME_ZONE`]) y se convierten a UTC con la fecha de la cabecera. Si una hora cae en
//!   el cambio de hora (inexistente o ambigua), la lectura falla: no se adivina.
//! - El recorrido se valida (salida → … → meta) y se guarda sin salida ni meta.
//! - La fecha de nacimiento se lee para avanzar, pero no se guarda.
//! - Una etiqueta desconocida es un error, con su valor y su posición. Solo se tolera un último
//!   registro truncado que se reduzca a su etiqueta, como hace el lector de referencia.

use chrono::{
    DateTime, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, TimeZone, Utc,
};
use chrono_tz::Tz;
use thiserror::Error;

use crate::model::{
    Class, ControlCode, Course, Event, FINISH_CODE, Punch, RaceResult, RaceStatus, Runner,
    START_CODE, Sex,
};

/// Zona horaria de las horas del .spl. Todas las carreras del grupo son en España peninsular.
pub const RACE_TIME_ZONE: Tz = chrono_tz::Europe::Madrid;

const MAGIC: &[u8] = b"spl4";
/// `spl4` + 8 bytes sin identificar; las etiquetas de la cabecera empiezan detrás.
const PREAMBLE_LEN: usize = 12;
/// Marcador de fin de cabecera (sin valor). El primer registro de categoría va justo detrás.
const HEADER_END: u8 = 0x2c;
/// Etiqueta de inicio de un registro de categoría.
const CLASS_TAG: u8 = 0x40;
/// Hora de picada que indica que no se registró.
const MISSING_TIME: u32 = 0xFF_FFFF;

/// Errores al leer un .spl.
#[derive(Debug, Error, PartialEq)]
pub enum SplError {
    #[error("cabecera inválida: {0}")]
    InvalidHeader(&'static str),

    #[error("fecha de la carrera inválida en la cabecera: {0} (días OLE)")]
    InvalidEventDate(f64),

    #[error("a la cabecera le falta la fecha de la carrera (0x19)")]
    MissingEventDate,

    #[error(
        "la tabla de categorías (0x20) del byte {offset} va antes del número de categorías (0x1f)"
    )]
    ClassTableBeforeCount { offset: usize },

    #[error("no hay registros de categoría tras la cabecera")]
    NoClassRecords,

    #[error(
        "tras la cabecera (byte {offset}) se esperaba un registro de categoría (0x40) y hay 0x{tag:02x}"
    )]
    ExpectedClassRecord { tag: u8, offset: usize },

    #[error("la cabecera anuncia {expected} categorías (0x1f) y el fichero trae {found}")]
    ClassCountMismatch { expected: u16, found: usize },

    #[error("etiqueta desconocida 0x{tag:02x} en el byte {offset}")]
    UnknownTag { tag: u8, offset: usize },

    #[error("fichero truncado: el registro 0x{tag:02x} del byte {offset} no está completo")]
    Truncated { tag: u8, offset: usize },

    #[error(
        "etiqueta de corredor 0x{tag:02x} fuera de un registro de corredor en el byte {offset}"
    )]
    FieldOutsideRunner { tag: u8, offset: usize },

    #[error("bloque de tramos (0x47) de {len} bytes en el byte {offset}: no es múltiplo de 8")]
    InvalidLegBlock { len: usize, offset: usize },

    #[error("a la categoría {class_id} le falta el nombre (0x43)")]
    MissingClassName { class_id: u32 },

    #[error("al corredor {runner_id} le falta el estado (0x98)")]
    MissingStatus { runner_id: u32 },

    #[error("recorrido inválido en la categoría {class_id}: {reason}")]
    InvalidCourse { class_id: u32, reason: String },

    #[error(
        "la hora local {local} de la picada de la baliza {code} (byte {offset}) no existe en \
         {RACE_TIME_ZONE}: cae en el cambio de hora"
    )]
    NonexistentLocalTime {
        local: NaiveDateTime,
        code: ControlCode,
        offset: usize,
    },

    #[error(
        "la hora local {local} de la picada de la baliza {code} (byte {offset}) es ambigua en \
         {RACE_TIME_ZONE}: se repite en el cambio de hora"
    )]
    AmbiguousLocalTime {
        local: NaiveDateTime,
        code: ControlCode,
        offset: usize,
    },

    #[error("hora de picada fuera de rango: {centiseconds} centésimas (byte {offset})")]
    TimeOutOfRange { centiseconds: u32, offset: usize },
}

/// Lee un .spl completo y lo convierte al modelo de dominio.
pub fn read(data: &[u8]) -> Result<Event, SplError> {
    let header = read_header(data)?;
    match data.get(header.body_start) {
        None => return Err(SplError::NoClassRecords),
        Some(&CLASS_TAG) => {}
        Some(&tag) => {
            return Err(SplError::ExpectedClassRecord {
                tag,
                offset: header.body_start,
            });
        }
    }

    let mut parser = Parser {
        cursor: Cursor::new(data, header.body_start),
        date: header.date,
        classes: Vec::new(),
        in_runner: false,
    };
    parser.run()?;

    let found = parser.classes.len();
    if found == 0 {
        return Err(SplError::NoClassRecords);
    }
    if let Some(expected) = header.class_count
        && usize::from(expected) != found
    {
        return Err(SplError::ClassCountMismatch { expected, found });
    }

    let classes = parser
        .classes
        .into_iter()
        .map(ClassDraft::finish)
        .collect::<Result<_, _>>()?;
    Ok(Event {
        name: header.name,
        date: header.date,
        classes,
    })
}

/// Lo que el importador usa de la cabecera.
#[derive(Debug, PartialEq)]
struct Header {
    /// Nombre de la prueba (`0x14`).
    name: Option<String>,
    /// Día de la carrera (`0x19`).
    date: NaiveDate,
    /// Número de categorías (`0x1f`), si la cabecera lo trae.
    class_count: Option<u16>,
    /// Posición del primer registro de categoría: el byte siguiente al marcador `0x2c`.
    body_start: usize,
}

/// Recorre la cabecera por etiquetas (`docs/formato-spl.md`) hasta el marcador `0x2c`.
///
/// Las etiquetas que no se usan se leen para avanzar; una desconocida es un error con su
/// posición, como en el resto del fichero.
fn read_header(data: &[u8]) -> Result<Header, SplError> {
    if data.get(..MAGIC.len()) != Some(MAGIC) {
        return Err(SplError::InvalidHeader("no empieza por `spl4`"));
    }
    if data.len() < PREAMBLE_LEN {
        return Err(SplError::InvalidHeader("termina dentro del preámbulo"));
    }
    let mut c = Cursor::new(data, PREAMBLE_LEN);
    let mut name = None;
    let mut date = None;
    let mut class_count = None;
    loop {
        let (tag, offset) = c.tag().ok_or(SplError::InvalidHeader(
            "termina antes del marcador de fin de cabecera (0x2c)",
        ))?;
        match tag {
            0x14 => name = Some(c.text()?),
            // Organizador, país y textos del software de origen.
            0x18 | 0x1b | 0x23 | 0x25 | 0x26 => {
                c.text()?;
            }
            0x19 => date = Some(ole_date(c.f64()?)?),
            // Fechas y horas de creación o subida del fichero.
            0x22 | 0x24 => {
                c.f64()?;
            }
            0x27 => {
                c.u8()?;
            }
            0x1f => class_count = Some(c.u16()?),
            0x2b => {
                c.u16()?;
            }
            0x21 | 0x28 | 0x29 => {
                c.u32()?;
            }
            // Tabla de categorías: n × (desplazamiento u32, tamaño u32), con n = `0x1f`.
            0x20 => {
                let count = class_count.ok_or(SplError::ClassTableBeforeCount { offset })?;
                c.take(usize::from(count) * 8)?;
            }
            HEADER_END => break,
            _ => return Err(SplError::UnknownTag { tag, offset }),
        }
    }
    Ok(Header {
        name,
        date: date.ok_or(SplError::MissingEventDate)?,
        class_count,
        body_start: c.pos,
    })
}

/// Convierte una hora del .spl (centésimas desde la medianoche local del día `date`) a UTC.
///
/// Las horas de 24 h o más caen en los días siguientes. Una hora inexistente o ambigua por el
/// cambio de hora es un error (ver `docs/formato-spl.md`).
fn local_time_to_utc(
    date: NaiveDate,
    centiseconds: u32,
    code: ControlCode,
    offset: usize,
) -> Result<DateTime<Utc>, SplError> {
    let local = TimeDelta::try_milliseconds(i64::from(centiseconds) * 10)
        .and_then(|delta| date.and_time(NaiveTime::MIN).checked_add_signed(delta))
        .ok_or(SplError::TimeOutOfRange {
            centiseconds,
            offset,
        })?;
    match RACE_TIME_ZONE.from_local_datetime(&local) {
        LocalResult::Single(t) => Ok(t.with_timezone(&Utc)),
        LocalResult::Ambiguous(_, _) => Err(SplError::AmbiguousLocalTime {
            local,
            code,
            offset,
        }),
        LocalResult::None => Err(SplError::NonexistentLocalTime {
            local,
            code,
            offset,
        }),
    }
}

/// Fecha OLE (días desde 1899-12-30, la parte fraccionaria es la hora) → día.
fn ole_date(days: f64) -> Result<NaiveDate, SplError> {
    let invalid = SplError::InvalidEventDate(days);
    if !days.is_finite() || days.abs() > 3_000_000.0 {
        return Err(invalid);
    }
    // Comprobado arriba: cabe de sobra en un i64.
    let whole = days.floor() as i64;
    NaiveDate::from_ymd_opt(1899, 12, 30)
        .zip(TimeDelta::try_days(whole))
        .and_then(|(epoch, delta)| epoch.checked_add_signed(delta))
        .ok_or(invalid)
}

/// Correspondencia de estados (`docs/formato-spl.md`).
fn status_from_code(code: u8) -> RaceStatus {
    match code {
        0 => RaceStatus::Ok,
        6 => RaceStatus::NotClassified,
        10 => RaceStatus::DidNotStart,
        other => RaceStatus::Unknown(other),
    }
}

fn sex_from_code(code: u8) -> Option<Sex> {
    match code {
        1 => Some(Sex::Male),
        2 => Some(Sex::Female),
        _ => None,
    }
}

/// `0` significa «sin valor» en dorsal, tarjeta y puesto.
fn non_zero_u32(value: u32) -> Option<u32> {
    (value != 0).then_some(value)
}

/// Lecturas comprobadas sobre el fichero. Cualquier lectura que se salga del final devuelve
/// `SplError::Truncated` con la etiqueta y la posición del registro en curso.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
    tag: u8,
    tag_offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8], pos: usize) -> Self {
        Self {
            data,
            pos,
            tag: 0,
            tag_offset: pos,
        }
    }

    fn truncated(&self) -> SplError {
        SplError::Truncated {
            tag: self.tag,
            offset: self.tag_offset,
        }
    }

    /// Lee la etiqueta del siguiente registro. `None` solo al final del fichero.
    fn tag(&mut self) -> Option<(u8, usize)> {
        let offset = self.pos;
        let tag = *self.data.get(offset)?;
        self.pos = offset + 1;
        self.tag = tag;
        self.tag_offset = offset;
        Some((tag, offset))
    }

    /// Como [`Cursor::tag`], pero también devuelve `None` si el último registro se reduce a su
    /// etiqueta (registro final truncado, tolerado en el cuerpo del fichero).
    fn next_tag(&mut self) -> Option<(u8, usize)> {
        self.tag().filter(|_| self.pos < self.data.len())
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], SplError> {
        let bytes = self
            .pos
            .checked_add(n)
            .and_then(|end| self.data.get(self.pos..end))
            .ok_or_else(|| self.truncated())?;
        self.pos += n;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], SplError> {
        let bytes = self.take(N)?;
        <[u8; N]>::try_from(bytes).map_err(|_| self.truncated())
    }

    fn u8(&mut self) -> Result<u8, SplError> {
        let [b] = self.array::<1>()?;
        Ok(b)
    }

    fn u16(&mut self) -> Result<u16, SplError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u24(&mut self) -> Result<u32, SplError> {
        let [b0, b1, b2] = self.array::<3>()?;
        Ok(u32::from_le_bytes([b0, b1, b2, 0]))
    }

    fn u32(&mut self) -> Result<u32, SplError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn f64(&mut self) -> Result<f64, SplError> {
        Ok(f64::from_le_bytes(self.array()?))
    }

    /// Texto: `u16` longitud + bytes Latin-1 (cada byte es el carácter Unicode del mismo valor).
    fn text(&mut self) -> Result<String, SplError> {
        let len = usize::from(self.u16()?);
        Ok(self.take(len)?.iter().map(|&b| char::from(b)).collect())
    }
}

struct ClassDraft {
    id: u32,
    name: Option<String>,
    short_name: Option<String>,
    legs: Option<Vec<(ControlCode, ControlCode)>>,
    runners: Vec<RunnerDraft>,
}

impl ClassDraft {
    fn finish(self) -> Result<Class, SplError> {
        let name = self
            .name
            .ok_or(SplError::MissingClassName { class_id: self.id })?;
        let course = course_from_legs(self.id, self.legs.as_deref().unwrap_or_default())?;
        let results = self
            .runners
            .into_iter()
            .map(RunnerDraft::finish)
            .collect::<Result<_, _>>()?;
        Ok(Class {
            id: self.id,
            name,
            short_name: self.short_name,
            course,
            results,
        })
    }
}

/// Valida la cadena de tramos (salida → … → meta) y devuelve las balizas sin salida ni meta.
fn course_from_legs(
    class_id: u32,
    legs: &[(ControlCode, ControlCode)],
) -> Result<Course, SplError> {
    let invalid = |reason: String| SplError::InvalidCourse { class_id, reason };
    let (Some(&(first_from, _)), Some((&(_, last_to), inner))) = (legs.first(), legs.split_last())
    else {
        return Err(invalid("no tiene tramos (0x47)".into()));
    };
    if first_from != START_CODE {
        return Err(invalid(format!(
            "el primer tramo sale de {first_from}, no de la salida ({START_CODE})"
        )));
    }
    let broken = legs
        .iter()
        .zip(legs.iter().skip(1))
        .find(|((_, to), (from, _))| to != from);
    if let Some(((_, to), (from, _))) = broken {
        return Err(invalid(format!(
            "un tramo acaba en {to} y el siguiente sale de {from}"
        )));
    }
    if last_to != FINISH_CODE {
        return Err(invalid(format!(
            "el último tramo acaba en {last_to}, no en la meta ({FINISH_CODE})"
        )));
    }
    let controls: Vec<ControlCode> = inner.iter().map(|&(_, to)| to).collect();
    if let Some(code) = controls
        .iter()
        .find(|&&c| c == START_CODE || c == FINISH_CODE)
    {
        return Err(invalid(format!(
            "la baliza especial {code} aparece a mitad del recorrido"
        )));
    }
    Ok(Course { controls })
}

struct RunnerDraft {
    id: u32,
    given_name: Option<String>,
    family_name: Option<String>,
    club: Option<String>,
    bib: Option<u32>,
    si_card: Option<u32>,
    status: Option<u8>,
    place: Option<u16>,
    sex: Option<Sex>,
    punches: Vec<Punch>,
}

impl RunnerDraft {
    fn new(id: u32) -> Self {
        Self {
            id,
            given_name: None,
            family_name: None,
            club: None,
            bib: None,
            si_card: None,
            status: None,
            place: None,
            sex: None,
            punches: Vec::new(),
        }
    }

    fn finish(self) -> Result<RaceResult, SplError> {
        let status = self
            .status
            .map(status_from_code)
            .ok_or(SplError::MissingStatus { runner_id: self.id })?;
        Ok(RaceResult {
            runner: Runner {
                id: self.id,
                given_name: self.given_name.unwrap_or_default(),
                family_name: self.family_name.unwrap_or_default(),
                club: self.club,
                bib: self.bib,
                si_card: self.si_card,
                sex: self.sex,
            },
            status,
            place: self.place,
            punches: self.punches,
        })
    }
}

struct Parser<'a> {
    cursor: Cursor<'a>,
    date: NaiveDate,
    classes: Vec<ClassDraft>,
    in_runner: bool,
}

impl Parser<'_> {
    fn run(&mut self) -> Result<(), SplError> {
        while let Some((tag, offset)) = self.cursor.next_tag() {
            self.record(tag, offset)?;
        }
        Ok(())
    }

    /// Categoría en curso. La lectura empieza en un `0x40`, así que siempre hay una; si no, se
    /// trata como etiqueta fuera de lugar en vez de entrar en pánico.
    fn class(&mut self, tag: u8, offset: usize) -> Result<&mut ClassDraft, SplError> {
        self.classes
            .last_mut()
            .ok_or(SplError::UnknownTag { tag, offset })
    }

    fn runner(&mut self, tag: u8, offset: usize) -> Result<&mut RunnerDraft, SplError> {
        let in_runner = self.in_runner;
        self.classes
            .last_mut()
            .and_then(|class| class.runners.last_mut())
            .filter(|_| in_runner)
            .ok_or(SplError::FieldOutsideRunner { tag, offset })
    }

    fn record(&mut self, tag: u8, offset: usize) -> Result<(), SplError> {
        let c = &mut self.cursor;
        match tag {
            // --- Categoría ---
            0x40 => {
                let id = c.u32()?;
                self.classes.push(ClassDraft {
                    id,
                    name: None,
                    short_name: None,
                    legs: None,
                    runners: Vec::new(),
                });
                self.in_runner = false;
            }
            0x43 => {
                let name = c.text()?;
                self.class(tag, offset)?.name = Some(name);
            }
            0x44 => {
                let short_name = c.text()?;
                self.class(tag, offset)?.short_name = Some(short_name);
            }
            0x47 => {
                let len = usize::try_from(c.u32()?).map_err(|_| c.truncated())?;
                let raw = c.take(len)?;
                if len % 8 != 0 {
                    return Err(SplError::InvalidLegBlock { len, offset });
                }
                let legs = raw
                    .chunks_exact(8)
                    .map(|leg| match leg {
                        // desde u16, hasta u16, longitud u32 (a 0 en los ejemplos; se ignora)
                        [f0, f1, t0, t1, ..] => (
                            u16::from_le_bytes([*f0, *f1]),
                            u16::from_le_bytes([*t0, *t1]),
                        ),
                        _ => (0, 0),
                    })
                    .collect();
                self.class(tag, offset)?.legs = Some(legs);
            }
            // Campos de categoría sin interpretar: se leen para avanzar.
            0x48 | 0x49 | 0x4a | 0x4e => {
                c.u8()?;
            }
            0x45 | 0x4f | 0x50 => {
                c.u16()?;
            }
            0x4d => {
                c.u32()?;
            }

            // --- Corredor ---
            0x80 => {
                let id = c.u32()?;
                self.class(tag, offset)?.runners.push(RunnerDraft::new(id));
                self.in_runner = true;
            }
            0x81 => {
                let bib = non_zero_u32(c.u32()?);
                self.runner(tag, offset)?.bib = bib;
            }
            0x84 => {
                let si_card = non_zero_u32(c.u32()?);
                self.runner(tag, offset)?.si_card = si_card;
            }
            0x87 => {
                let given = c.text()?;
                self.runner(tag, offset)?.given_name = Some(given);
            }
            0x88 => {
                let family = c.text()?;
                self.runner(tag, offset)?.family_name = Some(family);
            }
            0x8c => {
                let club = c.text()?;
                self.runner(tag, offset)?.club = Some(club);
            }
            0x97 => {
                let punches = self.punches()?;
                self.runner(tag, offset)?.punches = punches;
            }
            0x98 => {
                let status = c.u8()?;
                self.runner(tag, offset)?.status = Some(status);
            }
            0x99 => {
                let place = c.u16()?;
                self.runner(tag, offset)?.place = (place != 0).then_some(place);
            }
            0x9a => {
                let sex = sex_from_code(c.u8()?);
                self.runner(tag, offset)?.sex = sex;
            }
            // Campos de corredor que no guardamos: id de club (0x89), país (0x8d),
            // nacionalidad (0x8e) y fecha de nacimiento (0x9b, se descarta siempre).
            0x89 => {
                c.u32()?;
                self.runner(tag, offset)?;
            }
            0x8d | 0x8e => {
                c.text()?;
                self.runner(tag, offset)?;
            }
            0x9b => {
                c.f64()?;
                self.runner(tag, offset)?;
            }

            _ => return Err(SplError::UnknownTag { tag, offset }),
        }
        Ok(())
    }

    /// Picadas (0x97): `u16` n + n × (código `u16`, hora `u24`).
    fn punches(&mut self) -> Result<Vec<Punch>, SplError> {
        let count = self.cursor.u16()?;
        let mut punches = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            let offset = self.cursor.pos;
            let code = self.cursor.u16()?;
            let centiseconds = self.cursor.u24()?;
            let time = if centiseconds == MISSING_TIME {
                None
            } else {
                Some(local_time_to_utc(self.date, centiseconds, code, offset)?)
            };
            punches.push(Punch { code, time });
        }
        Ok(punches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Construcción de ficheros sintéticos ---

    fn ole_days(date: NaiveDate) -> f64 {
        let epoch = NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
        (date - epoch).num_days() as f64
    }

    struct SplBuilder(Vec<u8>);

    impl SplBuilder {
        /// `spl4` y el preámbulo de 8 bytes, sin etiquetas de cabecera.
        fn preamble() -> Self {
            let mut bytes = b"spl4".to_vec();
            bytes.extend([0x10, 0xdc, 0, 0, 0, 0, 0, 0]);
            Self(bytes)
        }
        /// Cabecera mínima: nombre, fecha y marcador de fin, sin número de categorías.
        fn new(date: NaiveDate) -> Self {
            Self::preamble()
                .text(0x14, "Trofeo de prueba")
                .f64(0x19, ole_days(date))
                .end_header()
        }
        /// Cabecera completa, con las etiquetas en el orden de los ficheros reales.
        fn full_header(name: &str, date: NaiveDate, class_count: u16) -> Self {
            let days = ole_days(date);
            Self::preamble()
                .text(0x14, name)
                .text(0x18, "Club Organizador")
                .text(0x1b, "ESP")
                .f64(0x19, days)
                .f64(0x22, days + 0.58)
                .text(0x23, "WinSplits Online Upload 4.0")
                .f64(0x24, days + 0.59)
                .text(0x25, "WinSplits Online Upload 4.0")
                .text(0x26, "IOFXML3 / Programa de prueba")
                .u8(0x27, 3)
                .u32(0x28, 1234)
                .u32(0x29, 0)
                .u16(0x1f, class_count)
                .u16(0x2b, 0)
                .u32(0x21, 0)
                .class_table(class_count)
                .end_header()
        }
        /// Tabla `0x20` de `n` pares (desplazamiento, tamaño). El lector no la valida.
        fn class_table(mut self, n: u16) -> Self {
            self.0.push(0x20);
            for _ in 0..n {
                self.0.extend([0; 8]);
            }
            self
        }
        fn end_header(mut self) -> Self {
            self.0.push(HEADER_END);
            self
        }
        fn u8(mut self, tag: u8, v: u8) -> Self {
            self.0.extend([tag, v]);
            self
        }
        fn u16(mut self, tag: u8, v: u16) -> Self {
            self.0.push(tag);
            self.0.extend(v.to_le_bytes());
            self
        }
        fn u32(mut self, tag: u8, v: u32) -> Self {
            self.0.push(tag);
            self.0.extend(v.to_le_bytes());
            self
        }
        fn f64(mut self, tag: u8, v: f64) -> Self {
            self.0.push(tag);
            self.0.extend(v.to_le_bytes());
            self
        }
        fn text(mut self, tag: u8, s: &str) -> Self {
            self.0.push(tag);
            let latin1: Vec<u8> = s.chars().map(|c| u8::try_from(c).unwrap()).collect();
            self.0.extend((latin1.len() as u16).to_le_bytes());
            self.0.extend(latin1);
            self
        }
        /// Tramos a partir de la secuencia completa salida, balizas…, meta.
        fn legs(mut self, codes: &[u16]) -> Self {
            self.0.push(0x47);
            self.0.extend(((codes.len() as u32 - 1) * 8).to_le_bytes());
            for pair in codes.windows(2) {
                self.0.extend(pair[0].to_le_bytes());
                self.0.extend(pair[1].to_le_bytes());
                self.0.extend(0u32.to_le_bytes());
            }
            self
        }
        fn punches(mut self, punches: &[(u16, u32)]) -> Self {
            self.0.push(0x97);
            self.0.extend((punches.len() as u16).to_le_bytes());
            for &(code, cs) in punches {
                self.0.extend(code.to_le_bytes());
                self.0.extend(&cs.to_le_bytes()[..3]);
            }
            self
        }
        fn class(self, id: u32, name: &str) -> Self {
            self.u32(0x40, id)
                .text(0x43, name)
                .text(0x44, name)
                .u16(0x45, 0)
                .u8(0x4e, 0)
                .legs(&[START_CODE, 31, 45, FINISH_CODE])
                .u8(0x48, 0)
                .u32(0x4d, 0)
        }
        fn runner(self, id: u32, status: u8, punches: &[(u16, u32)]) -> Self {
            self.u32(0x80, id)
                .u32(0x81, 100 + id)
                .u32(0x84, 2_000_000 + id)
                .text(0x87, "Ana")
                .text(0x88, "Pérez")
                .u32(0x89, 1)
                .text(0x8c, "Club A")
                .text(0x8d, "ESP")
                .text(0x8e, "ESP")
                .punches(punches)
                .u8(0x98, status)
                .u16(0x99, if status == 0 { 1 } else { 0 })
                .u8(0x9a, 2)
                .f64(0x9b, 30_000.0)
        }
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32, ms: u32) -> DateTime<Utc> {
        date(y, mo, d)
            .and_hms_milli_opt(h, mi, s, ms)
            .unwrap()
            .and_utc()
    }

    /// Centésimas desde la medianoche.
    fn cs(h: u32, m: u32, s: u32, hundredths: u32) -> u32 {
        ((h * 60 + m) * 60 + s) * 100 + hundredths
    }

    fn sample() -> SplBuilder {
        SplBuilder::new(date(2026, 10, 3)).class(7, "F21A").runner(
            1,
            0,
            &[
                (START_CODE, cs(10, 0, 0, 0)),
                (31, cs(10, 1, 30, 25)),
                (45, MISSING_TIME),
                (FINISH_CODE, cs(10, 4, 0, 0)),
            ],
        )
    }

    // --- Conversión de horas ---

    #[test]
    fn summer_time_is_utc_plus_two() {
        // 3 de octubre de 2026, horario de verano (CEST, UTC+2): 17:31:00 → 15:31:00Z.
        let t = local_time_to_utc(date(2026, 10, 3), 6_306_000, 31, 0).unwrap();
        assert_eq!(t, utc(2026, 10, 3, 15, 31, 0, 0));
    }

    #[test]
    fn winter_time_is_utc_plus_one() {
        // 12 de diciembre de 2026, horario de invierno (CET, UTC+1): 10:00:00,12 → 09:00:00,12Z.
        let t = local_time_to_utc(date(2026, 12, 12), 3_600_012, 31, 0).unwrap();
        assert_eq!(t, utc(2026, 12, 12, 9, 0, 0, 120));
    }

    #[test]
    fn local_midnight_and_next_day() {
        // 00:30 locales del 1 de enero = 23:30Z del 31 de diciembre.
        let t = local_time_to_utc(date(2027, 1, 1), cs(0, 30, 0, 0), 31, 0).unwrap();
        assert_eq!(t, utc(2026, 12, 31, 23, 30, 0, 0));
        // 25:00 (pasada la medianoche) cae en el día siguiente: 01:00 CET del 2 = 00:00Z del 2.
        let t = local_time_to_utc(date(2027, 1, 1), cs(25, 0, 0, 0), 31, 0).unwrap();
        assert_eq!(t, utc(2027, 1, 2, 0, 0, 0, 0));
    }

    #[test]
    fn nonexistent_local_time_is_an_error() {
        // 29 de marzo de 2026: a las 02:00 se pasa a las 03:00; las 02:30 no existen.
        let err = local_time_to_utc(date(2026, 3, 29), cs(2, 30, 0, 0), 31, 99).unwrap_err();
        assert!(matches!(
            err,
            SplError::NonexistentLocalTime {
                code: 31,
                offset: 99,
                ..
            }
        ));
        assert!(err.to_string().contains("2026-03-29 02:30:00"), "{err}");
        // Justo antes y justo después del salto sí existen.
        assert_eq!(
            local_time_to_utc(date(2026, 3, 29), cs(1, 59, 59, 0), 31, 0).unwrap(),
            utc(2026, 3, 29, 0, 59, 59, 0)
        );
        assert_eq!(
            local_time_to_utc(date(2026, 3, 29), cs(3, 0, 0, 0), 31, 0).unwrap(),
            utc(2026, 3, 29, 1, 0, 0, 0)
        );
    }

    #[test]
    fn ambiguous_local_time_is_an_error() {
        // 25 de octubre de 2026: a las 03:00 se vuelve a las 02:00; las 02:30 ocurren dos veces.
        let err = local_time_to_utc(date(2026, 10, 25), cs(2, 30, 0, 0), 45, 7).unwrap_err();
        assert!(matches!(
            err,
            SplError::AmbiguousLocalTime {
                code: 45,
                offset: 7,
                ..
            }
        ));
    }

    #[test]
    fn dst_error_propagates_from_a_file() {
        let data = SplBuilder::new(date(2026, 10, 25))
            .class(7, "F21A")
            .runner(1, 0, &[(START_CODE, cs(2, 30, 0, 0))])
            .0;
        assert!(matches!(
            read(&data),
            Err(SplError::AmbiguousLocalTime {
                code: START_CODE,
                ..
            })
        ));
    }

    #[test]
    fn ole_dates() {
        assert_eq!(ole_date(46_298.0).unwrap(), date(2026, 10, 3));
        // La parte fraccionaria es la hora del día y no cambia la fecha.
        assert_eq!(ole_date(46_298.75).unwrap(), date(2026, 10, 3));
        assert!(ole_date(f64::NAN).is_err());
        assert!(ole_date(1e300).is_err());
    }

    // --- Lectura de ficheros sintéticos ---

    #[test]
    fn reads_a_synthetic_file() {
        let event = read(&sample().0).unwrap();
        assert_eq!(event.name.as_deref(), Some("Trofeo de prueba"));
        assert_eq!(event.date, date(2026, 10, 3));
        assert_eq!(event.classes.len(), 1);

        let class = &event.classes[0];
        assert_eq!(class.id, 7);
        assert_eq!(class.name, "F21A");
        assert_eq!(class.short_name.as_deref(), Some("F21A"));
        assert_eq!(class.course.controls, vec![31, 45]);

        let result = &class.results[0];
        assert_eq!(
            result.runner,
            Runner {
                id: 1,
                given_name: "Ana".into(),
                family_name: "Pérez".into(),
                club: Some("Club A".into()),
                bib: Some(101),
                si_card: Some(2_000_001),
                sex: Some(Sex::Female),
            }
        );
        assert_eq!(result.status, RaceStatus::Ok);
        assert_eq!(result.place, Some(1));
        assert_eq!(
            result.punches,
            vec![
                Punch {
                    code: START_CODE,
                    time: Some(utc(2026, 10, 3, 8, 0, 0, 0)),
                },
                Punch {
                    code: 31,
                    time: Some(utc(2026, 10, 3, 8, 1, 30, 250)),
                },
                Punch {
                    code: 45,
                    time: None,
                },
                Punch {
                    code: FINISH_CODE,
                    time: Some(utc(2026, 10, 3, 8, 4, 0, 0)),
                },
            ]
        );
    }

    #[test]
    fn status_sex_and_zero_values() {
        assert_eq!(status_from_code(0), RaceStatus::Ok);
        assert_eq!(status_from_code(6), RaceStatus::NotClassified);
        assert_eq!(status_from_code(10), RaceStatus::DidNotStart);
        assert_eq!(status_from_code(3), RaceStatus::Unknown(3));
        assert_eq!(sex_from_code(1), Some(Sex::Male));
        assert_eq!(sex_from_code(2), Some(Sex::Female));
        assert_eq!(sex_from_code(0), None);
        assert_eq!(sex_from_code(9), None);

        let data = SplBuilder::new(date(2026, 10, 3))
            .class(7, "F21A")
            .u32(0x80, 1)
            .u32(0x81, 0)
            .u32(0x84, 0)
            .u8(0x98, 10)
            .u16(0x99, 0)
            .u8(0x9a, 0)
            .0;
        let result = &read(&data).unwrap().classes[0].results[0];
        assert_eq!(result.status, RaceStatus::DidNotStart);
        assert_eq!(result.place, None);
        assert_eq!(result.runner.bib, None);
        assert_eq!(result.runner.si_card, None);
        assert_eq!(result.runner.sex, None);
        assert_eq!(result.runner.given_name, "");
        assert!(result.punches.is_empty());
    }

    #[test]
    fn tolerates_a_last_record_reduced_to_its_tag() {
        // Como en el fixture de Baltanás: el fichero acaba en la etiqueta de sexo, sin valor.
        let mut data = SplBuilder::new(date(2026, 10, 3))
            .class(7, "F21A")
            .u32(0x80, 1)
            .u8(0x98, 0)
            .0;
        data.push(0x9a);
        let result = &read(&data).unwrap().classes[0].results[0];
        assert_eq!(result.runner.sex, None);
    }

    // --- Ficheros corruptos: siempre `Err`, nunca pánico ---

    #[test]
    fn bad_header_is_an_error() {
        let mut data = sample().0;
        data[..4].copy_from_slice(b"spl3");
        assert!(matches!(read(&data), Err(SplError::InvalidHeader(_))));
        assert!(matches!(read(b""), Err(SplError::InvalidHeader(_))));
        assert!(matches!(read(b"spl4"), Err(SplError::InvalidHeader(_))));
        // Cabecera completa pero sin ningún registro de categoría.
        let header = SplBuilder::new(date(2026, 10, 3)).0;
        assert_eq!(read(&header), Err(SplError::NoClassRecords));
    }

    // --- Cabecera ---

    #[test]
    fn header_is_read_by_tags_wherever_the_date_falls() {
        // Nombre más corto que el de Baltanás: la fecha (0x19) ya no cae en el offset 0x4C, como
        // en el segundo fichero real que hizo fallar la lectura por offset fijo.
        let data = SplBuilder::full_header("Liga de prueba 2026", date(2026, 7, 19), 2)
            .class(7, "F21A")
            .runner(1, 0, &[(START_CODE, cs(10, 0, 0, 0))])
            .class(8, "M21A")
            .0;
        let at_0x4c = f64::from_le_bytes(data[0x4C..0x54].try_into().unwrap());
        assert_ne!(at_0x4c, ole_days(date(2026, 7, 19)));

        let header = read_header(&data).unwrap();
        assert_eq!(header.class_count, Some(2));
        assert_eq!(data[header.body_start], CLASS_TAG);
        assert_eq!(data[header.body_start - 1], HEADER_END);

        let event = read(&data).unwrap();
        assert_eq!(event.name.as_deref(), Some("Liga de prueba 2026"));
        assert_eq!(event.date, date(2026, 7, 19));
        assert_eq!(event.classes.len(), 2);
        // 10:00 del 19 de julio (CEST) → 08:00Z.
        assert_eq!(
            event.classes[0].results[0].punches[0].time,
            Some(utc(2026, 7, 19, 8, 0, 0, 0))
        );
    }

    #[test]
    fn unknown_header_tag_reports_its_position() {
        let mut data = SplBuilder::preamble().text(0x14, "Trofeo").0;
        let offset = data.len();
        data.extend([0x15, 0, 0]);
        let data = SplBuilder(data)
            .f64(0x19, ole_days(date(2026, 10, 3)))
            .end_header()
            .class(7, "F21A")
            .0;
        assert_eq!(read(&data), Err(SplError::UnknownTag { tag: 0x15, offset }));
    }

    #[test]
    fn missing_event_date_is_an_error() {
        let data = SplBuilder::preamble()
            .text(0x14, "Trofeo")
            .end_header()
            .class(7, "F21A")
            .0;
        assert_eq!(read(&data), Err(SplError::MissingEventDate));
    }

    #[test]
    fn header_without_end_marker_is_an_error() {
        let data = SplBuilder::preamble()
            .text(0x14, "Trofeo")
            .f64(0x19, ole_days(date(2026, 10, 3)))
            .0;
        assert!(matches!(read(&data), Err(SplError::InvalidHeader(_))));
        // Cortada a mitad de un valor: se informa del registro.
        let cut = &data[..data.len() - 3];
        assert_eq!(
            read(cut),
            Err(SplError::Truncated {
                tag: 0x19,
                offset: data.len() - 9,
            })
        );
    }

    #[test]
    fn class_count_must_match_the_header() {
        let data = SplBuilder::full_header("Trofeo", date(2026, 10, 3), 2)
            .class(7, "F21A")
            .0;
        assert_eq!(
            read(&data),
            Err(SplError::ClassCountMismatch {
                expected: 2,
                found: 1
            })
        );
    }

    #[test]
    fn class_table_needs_the_class_count_first() {
        let builder = SplBuilder::preamble().f64(0x19, ole_days(date(2026, 10, 3)));
        let offset = builder.0.len();
        let data = builder.class_table(0).end_header().class(7, "F21A").0;
        assert_eq!(read(&data), Err(SplError::ClassTableBeforeCount { offset }));
    }

    #[test]
    fn body_must_start_with_a_class_record() {
        let builder = SplBuilder::new(date(2026, 10, 3));
        let offset = builder.0.len();
        let data = builder.u8(0x48, 0).class(7, "F21A").0;
        assert_eq!(
            read(&data),
            Err(SplError::ExpectedClassRecord { tag: 0x48, offset })
        );
    }

    #[test]
    fn unknown_tag_reports_value_and_position() {
        let mut data = sample().0;
        let offset = data.len();
        data.extend([0x7f, 0, 0]);
        let err = read(&data).unwrap_err();
        assert_eq!(err, SplError::UnknownTag { tag: 0x7f, offset });
        assert_eq!(
            err.to_string(),
            format!("etiqueta desconocida 0x7f en el byte {offset}")
        );
    }

    #[test]
    fn truncated_in_the_middle_of_a_record_is_an_error() {
        let data = sample().0;
        // Se corta a mitad del texto del nombre del corredor ("Ana").
        let name_at = data
            .windows(6)
            .position(|w| w == [0x87, 3, 0, b'A', b'n', b'a'])
            .unwrap();
        let cut = &data[..name_at + 4];
        assert_eq!(
            read(cut),
            Err(SplError::Truncated {
                tag: 0x87,
                offset: name_at,
            })
        );
    }

    #[test]
    fn every_prefix_and_bit_flip_returns_without_panicking() {
        let data = sample().0;
        for len in 0..data.len() {
            let _ = read(&data[..len]);
        }
        for i in 0..data.len() {
            for bit in 0..8 {
                let mut corrupt = data.clone();
                corrupt[i] ^= 1 << bit;
                let _ = read(&corrupt);
            }
        }
    }

    #[test]
    fn invalid_courses_are_rejected() {
        let no_finish = SplBuilder::new(date(2026, 10, 3))
            .u32(0x40, 7)
            .text(0x43, "F21A")
            .legs(&[START_CODE, 31, 45])
            .0;
        assert!(matches!(
            read(&no_finish),
            Err(SplError::InvalidCourse { class_id: 7, .. })
        ));

        let broken_chain = SplBuilder::new(date(2026, 10, 3))
            .u32(0x40, 7)
            .text(0x43, "F21A")
            .legs(&[START_CODE, 31, FINISH_CODE])
            .0;
        // Se rompe la cadena: el segundo tramo sale de 32 en lugar de 31.
        let mut broken_chain = broken_chain;
        let len = broken_chain.len();
        broken_chain[len - 8..len - 6].copy_from_slice(&32u16.to_le_bytes());
        assert!(matches!(
            read(&broken_chain),
            Err(SplError::InvalidCourse { class_id: 7, .. })
        ));

        let without_legs = SplBuilder::new(date(2026, 10, 3))
            .u32(0x40, 7)
            .text(0x43, "F21A")
            .0;
        assert!(matches!(
            read(&without_legs),
            Err(SplError::InvalidCourse { class_id: 7, .. })
        ));
    }

    #[test]
    fn runner_fields_outside_a_runner_are_an_error() {
        let data = SplBuilder::new(date(2026, 10, 3))
            .class(7, "F21A")
            .text(0x87, "Ana")
            .0;
        assert!(matches!(
            read(&data),
            Err(SplError::FieldOutsideRunner { tag: 0x87, .. })
        ));
    }

    #[test]
    fn missing_status_is_an_error() {
        let data = SplBuilder::new(date(2026, 10, 3))
            .class(7, "F21A")
            .u32(0x80, 1)
            .text(0x87, "Ana")
            .0;
        assert_eq!(read(&data), Err(SplError::MissingStatus { runner_id: 1 }));
    }
}
