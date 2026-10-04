mod analyze;
mod table;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use tramos_core::importers::{fit, spl};
use tramos_core::lost_time::{
    DEFAULT_ERROR_THRESHOLD_PCT, DEFAULT_ERROR_THRESHOLD_S, IdealTime, LostTimeConfig,
};

use crate::analyze::{RunnerQuery, analyze};

/// CLI de desarrollo de Tramos: análisis de carreras de orientación tramo a tramo.
#[derive(Debug, Parser)]
#[command(name = "tramos", version = tramos_core::VERSION, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Tiempo perdido de un corredor y, con FIT, la alineación del reloj con sus picadas.
    Analizar(AnalyzeArgs),
}

#[derive(Debug, Args)]
struct AnalyzeArgs {
    /// Splits de WinSplits (.spl).
    #[arg(long, value_name = "FICHERO")]
    spl: PathBuf,

    /// Actividad del reloj del corredor (.fit).
    #[arg(long, value_name = "FICHERO")]
    fit: Option<PathBuf>,

    /// Corredor: tarjeta SI si es un número; si no, nombre y apellidos completos.
    #[arg(long, value_name = "NOMBRE|TARJETA")]
    corredor: Option<String>,

    /// Pérdida mínima (s) para que un tramo sea error.
    #[arg(long, value_name = "SEGUNDOS", default_value_t = DEFAULT_ERROR_THRESHOLD_S)]
    umbral_s: f64,

    /// Pérdida mínima (% del tiempo esperado) para que un tramo sea error.
    #[arg(long, value_name = "PORCENTAJE", default_value_t = DEFAULT_ERROR_THRESHOLD_PCT)]
    umbral_pct: f64,

    /// Definición del tiempo ideal.
    #[arg(long, value_enum, default_value_t = Ideal::SumaReferencias)]
    ideal: Ideal,

    /// Formato de la salida.
    #[arg(long, value_enum, default_value_t = Format::Json)]
    formato: Format,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Ideal {
    /// Suma de las referencias de los tramos.
    SumaReferencias,
    /// Suma de los mejores splits de cada tramo (el "superman" de WinSplits).
    SumaMejores,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Format {
    Json,
    Tabla,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Analizar(args) => run_analyze(&args),
    }
}

fn run_analyze(args: &AnalyzeArgs) -> Result<()> {
    let Some(corredor) = &args.corredor else {
        bail!("falta --corredor <NOMBRE|TARJETA>: la tarjeta SI o el nombre y apellidos");
    };
    let query = RunnerQuery::parse(corredor);
    if query == RunnerQuery::Name(String::new()) {
        bail!("--corredor está vacío: pon la tarjeta SI o el nombre y apellidos");
    }
    let config = LostTimeConfig {
        error_threshold_s: threshold(args.umbral_s, "--umbral-s")?,
        error_threshold_pct: threshold(args.umbral_pct, "--umbral-pct")?,
        ideal_time: match args.ideal {
            Ideal::SumaReferencias => IdealTime::SumOfReferences,
            Ideal::SumaMejores => IdealTime::SumOfBestSplits,
        },
    };

    let event = spl::read(&read_file(&args.spl)?)
        .with_context(|| format!("no se puede leer el .spl {}", args.spl.display()))?;
    let track = match &args.fit {
        Some(path) => Some(
            fit::read(&read_file(path)?)
                .with_context(|| format!("no se puede leer el FIT {}", path.display()))?,
        ),
        None => None,
    };

    let analysis = analyze(&event, &query, &config, track.as_ref())?;
    match args.formato {
        Format::Json => {
            let json = serde_json::to_string_pretty(&analysis)
                .context("no se puede convertir el análisis a JSON")?;
            println!("{json}");
        }
        Format::Tabla => print!("{}", table::render(&analysis)),
    }
    Ok(())
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("no se puede abrir {}", path.display()))
}

fn threshold(value: f64, flag: &str) -> Result<f64> {
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        bail!("{flag} tiene que ser un número mayor o igual que 0 (es {value})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn thresholds_must_be_non_negative() {
        assert_eq!(threshold(15.0, "--umbral-s").unwrap(), 15.0);
        assert!(threshold(-1.0, "--umbral-s").is_err());
        assert!(threshold(f64::NAN, "--umbral-s").is_err());
    }
}
