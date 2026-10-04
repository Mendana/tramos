// Evita la consola adicional en Windows en compilaciones de release. NO QUITAR.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    match tramos_app_lib::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error al ejecutar Tramos: {err}");
            ExitCode::FAILURE
        }
    }
}
