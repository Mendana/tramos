use clap::Parser;

/// CLI de desarrollo de Tramos: análisis de carreras de orientación tramo a tramo.
#[derive(Debug, Parser)]
#[command(name = "tramos", version = tramos_core::VERSION, about)]
struct Cli {}

fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
