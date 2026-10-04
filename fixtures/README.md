# Fixtures

Datos de prueba **públicos**: solo anonimizados o sintéticos.

- Los datos reales van en `fixtures/private/`, que está en `.gitignore`.
- Para crear un fixture público a partir de uno real, usa las herramientas de anonimizado de
  `tools/` (issues de M0) y revisa el resultado antes de subirlo.
- Cada fixture lleva al lado un fichero `.expected.json` con la salida esperada, que usan los tests.

## Contenido

| Fichero | Qué es |
| --- | --- |
| `spl/baltanas-anon.spl` | Sprint de Baltanás (Liga Norte / Liga FOCYL), anonimizado: 18 categorías, 275 corredores, 9 recorridos. |
| `spl/baltanas-anon.expected.json` | Salida de `tools/reference/winsplits_spl.py` sobre el fichero anterior. |

Para regenerarlos desde el original (que solo existe en `fixtures/private/`):

```bash
python3 tools/anonimizar_spl.py fixtures/private/baltanas.spl fixtures/spl/baltanas-anon.spl
```
