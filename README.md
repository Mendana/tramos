# Tramos

Análisis de carreras de orientación a pie, tramo a tramo y carrera tras carrera.
Cruza el FIT de tu reloj con los splits de WinSplits, calcula dónde y por qué pierdes tiempo y
busca patrones de error en tu histórico. App de escritorio local (Tauri 2), de código abierto (MIT).

> Estado: en construcción. El trabajo se organiza en issues por milestone.

## Instalar

Descarga el instalador de Windows de la última versión en
[Releases](https://github.com/Mendana/tramos/releases) y ábrelo. Aún no está firmado, así que
Windows SmartScreen avisará de que no es de un editor reconocido: pulsa «Más información» y
«Ejecutar de todas formas». Más detalles, y cómo se publica una versión, en
[docs/distribucion.md](docs/distribucion.md).

## Documentación

- [Visión y decisiones](docs/vision.md)
- [Preguntas del MVP y cómo se calculan](docs/preguntas.md)
- [Tiempo perdido](docs/tiempo-perdido.md)
- [Formato .spl de WinSplits](docs/formato-spl.md)
- [Formato FIT del reloj](docs/formato-fit.md)
- [Identificar al corredor en una carrera](docs/identificacion.md)
- [CLI de desarrollo (`tramos analizar`)](docs/cli.md)
- [Taxonomía de errores](docs/taxonomia.md)
- [Datos y privacidad](docs/datos-y-privacidad.md)
- [Distribución: instalador y versiones](docs/distribucion.md)
- [Paquete por carrera (corredor → entrenadora)](docs/paquete.md)
- [Cómo trabajamos con agentes](docs/trabajo-con-agentes.md)

## Desarrollo

Requisitos: Rust estable, Node LTS y los [requisitos de Tauri 2](https://v2.tauri.app/start/prerequisites/).

```bash
cargo test --workspace
python3 tools/reference/winsplits_spl.py carrera.spl --csv   # lector de referencia
```

Mientras no hay interfaz, el análisis se puede hacer desde la terminal con la CLI
([docs/cli.md](docs/cli.md)):

```bash
cargo run -p tramos-cli -- analizar --spl carrera.spl --fit reloj.fit --corredor <tarjeta SI|nombre> --formato tabla
```

## Licencia

MIT. Ver [LICENSE](LICENSE).
