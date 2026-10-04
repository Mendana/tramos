//! App de escritorio de Tramos. Expone el núcleo a la interfaz mediante comandos Tauri;
//! no reimplementa cálculos.

/// Devuelve la versión del núcleo (`tramos_core::VERSION`).
#[tauri::command]
fn core_version() -> &'static str {
    tramos_core::VERSION
}

/// Arranca la app. Devuelve el error de Tauri en lugar de abortar.
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![core_version])
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_version_is_the_core_crate_version() {
        assert_eq!(core_version(), tramos_core::VERSION);
    }
}
