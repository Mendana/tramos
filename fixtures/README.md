# Fixtures

Datos de prueba **públicos**: solo anonimizados o sintéticos.

- Los datos reales van en `fixtures/private/`, que está en `.gitignore`.
- Para crear un fixture público a partir de uno real, usa las herramientas de anonimizado de
  `tools/` (issues de M0) y revisa el resultado antes de subirlo.
- Cada fixture lleva al lado un fichero `.expected.json` con la salida esperada, que usan los tests.
