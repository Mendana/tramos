//! `--formato tabla`: el análisis en texto para leerlo en la terminal.

use std::fmt::Write;

use tramos_core::lost_time::IdealTime;
use tramos_core::model::{ControlCode, FINISH_CODE, START_CODE};

use crate::analyze::{Analysis, status_label};

/// Valor que falta en una celda.
const MISSING: &str = "-";

pub fn render(analysis: &Analysis) -> String {
    let mut out = String::new();
    // Escribir en un `String` no falla: se ignora el `fmt::Result`.
    let _ = write_analysis(&mut out, analysis);
    out
}

fn write_analysis(out: &mut String, a: &Analysis) -> std::fmt::Result {
    let event_name = a.event.name.as_deref().unwrap_or("Carrera sin nombre");
    writeln!(out, "{event_name} · {}", a.event.date)?;

    let r = &a.runner;
    let card = r
        .si_card
        .map_or_else(|| "sin tarjeta".to_string(), |c| format!("tarjeta {c}"));
    writeln!(
        out,
        "Corredor: {} {} ({}, {card}) · {}",
        r.given_name,
        r.family_name,
        r.class_name,
        status_label(r.status, r.place)
    )?;

    let classes: Vec<&str> = a.course.classes.iter().map(|c| c.name.as_str()).collect();
    writeln!(
        out,
        "Recorrido: {} balizas, {} tramos · categorías: {} · {} clasificados",
        a.course.controls.len(),
        a.lost_time.legs.len(),
        classes.join(", "),
        a.course.valid_runners
    )?;
    let ideal = match a.config.ideal_time {
        IdealTime::SumOfReferences => "suma de referencias",
        IdealTime::SumOfBestSplits => "suma de mejores splits",
    };
    writeln!(
        out,
        "Error: pérdida > {} s y > {} % · tiempo ideal: {ideal}",
        decimal(a.config.error_threshold_s, 0),
        decimal(a.config.error_threshold_pct, 0)
    )?;
    writeln!(out)?;

    writeln!(
        out,
        "{:>5}  {:<11} {:>6} {:>6} {:>7} {:>9} {:>8}  Notas",
        "Tramo", "Balizas", "Split", "Ref.", "IR", "Pérdida", "%"
    )?;
    for leg in &a.lost_time.legs {
        let codes = format!("{}→{}", code_label(leg.from), code_label(leg.to));
        let mut notes = Vec::new();
        if leg.is_error {
            notes.push("ERROR");
        }
        if leg.is_last {
            notes.push("último");
        }
        if leg.short_reference {
            notes.push("ref. corta");
        }
        let row = format!(
            "{:>5}  {:<11} {:>6} {:>6} {:>7} {:>9} {:>8}  {}",
            leg.index,
            codes,
            opt(leg.split_s, clock),
            opt(leg.reference_s, clock),
            opt(leg.performance_index, |v| format!(
                "{} %",
                decimal(v * 100.0, 1)
            )),
            opt(leg.loss_s, |v| format!("{} s", signed(v))),
            opt(leg.loss_pct, |v| format!("{} %", signed(v))),
            notes.join(", ")
        );
        writeln!(out, "{}", row.trim_end())?;
    }
    writeln!(out)?;

    let t = &a.lost_time;
    writeln!(
        out,
        "Total {} · rendimiento habitual {} · {} error(es) · tiempo perdido {} · sin errores {}",
        opt(t.total_s, clock),
        opt(t.usual_performance, |v| format!(
            "{} %",
            decimal(v * 100.0, 1)
        )),
        t.error_count,
        opt(t.lost_time_s, clock),
        opt(t.time_without_errors_s, clock)
    )?;
    writeln!(
        out,
        "Tiempo ideal {} · diferencia en meta {}",
        opt(t.ideal_time_s, clock),
        opt(t.behind_ideal_s, signed_clock)
    )?;
    if a.course.weak_reference {
        writeln!(
            out,
            "Referencia débil: solo {} clasificados en el recorrido.",
            a.course.valid_runners
        )?;
    }

    if let Some(al) = &a.alignment {
        writeln!(out)?;
        let estimated = if al.offset_estimated {
            ""
        } else {
            " (no estimado)"
        };
        writeln!(
            out,
            "Alineación del FIT: desfase {} s{estimated} · confianza {} · {} picadas usadas \
             (apoyo {}, separación {})",
            signed(al.offset_s),
            decimal(al.confidence, 2),
            al.quality.controls_used,
            decimal(al.quality.support, 2),
            decimal(al.quality.margin, 2)
        )?;
        for warning in &al.warnings {
            writeln!(out, "  Aviso: {}", warning.message)?;
        }
    }

    if !a.warnings.is_empty() {
        writeln!(out)?;
        for warning in &a.warnings {
            writeln!(out, "Aviso: {warning}")?;
        }
    }
    Ok(())
}

fn code_label(code: ControlCode) -> String {
    match code {
        START_CODE => "S".to_string(),
        FINISH_CODE => "M".to_string(),
        code => code.to_string(),
    }
}

fn opt(value: Option<f64>, format: impl Fn(f64) -> String) -> String {
    value.map_or_else(|| MISSING.to_string(), format)
}

/// Número con `decimals` decimales y coma decimal.
fn decimal(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}").replace('.', ",")
}

/// Número con signo y un decimal: `+5,3`, `-2,0`.
fn signed(value: f64) -> String {
    format!("{value:+.1}").replace('.', ",")
}

/// Duración redondeada al segundo: `m:ss`, o `h:mm:ss` desde una hora.
fn clock(seconds: f64) -> String {
    let total = seconds.abs().round() as u64;
    let sign = if seconds < 0.0 && total > 0 { "-" } else { "" };
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 {
        format!("{sign}{h}:{m:02}:{s:02}")
    } else {
        format!("{sign}{m}:{s:02}")
    }
}

/// Como [`clock`], con `+` delante de las positivas.
fn signed_clock(seconds: f64) -> String {
    let text = clock(seconds);
    if text.starts_with('-') {
        text
    } else {
        format!("+{text}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_formats() {
        assert_eq!(clock(54.0), "0:54");
        assert_eq!(clock(427.4), "7:07");
        assert_eq!(clock(1540.0), "25:40");
        assert_eq!(clock(3725.0), "1:02:05");
        assert_eq!(clock(-12.6), "-0:13");
        assert_eq!(clock(-0.2), "0:00");
        assert_eq!(signed_clock(160.0), "+2:40");
        assert_eq!(signed_clock(-5.0), "-0:05");
    }

    #[test]
    fn numbers_use_decimal_comma() {
        assert_eq!(decimal(78.126, 1), "78,1");
        assert_eq!(signed(5.26), "+5,3");
        assert_eq!(signed(-2.0), "-2,0");
    }

    #[test]
    fn code_labels() {
        assert_eq!(code_label(START_CODE), "S");
        assert_eq!(code_label(FINISH_CODE), "M");
        assert_eq!(code_label(49), "49");
    }
}
