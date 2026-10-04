use std::process::Command;

#[test]
fn version_flag_prints_name_and_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_tramos"))
        .arg("--version")
        .output()
        .expect("no se pudo ejecutar el binario tramos");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("salida no UTF-8");
    assert_eq!(stdout.trim(), format!("tramos {}", tramos_core::VERSION));
}
