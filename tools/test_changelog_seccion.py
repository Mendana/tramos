"""Tests de la extracción de notas del CHANGELOG.

Ejecutar: python3 -m unittest discover -s tools -p "test_*.py"
"""
import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from changelog_seccion import seccion  # noqa: E402

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

EJEMPLO = """# Cambios

Introducción.

## Sin publicar

- Algo que aún no sale.

## 0.2.1

- Icono propio.

Más texto.

## 0.2.0

- Actualizaciones.
"""


class SeccionTest(unittest.TestCase):
    def test_saca_la_seccion_en_medio(self):
        self.assertEqual(seccion(EJEMPLO, "0.2.1"), "- Icono propio.\n\nMás texto.")

    def test_saca_la_ultima_seccion(self):
        self.assertEqual(seccion(EJEMPLO, "0.2.0"), "- Actualizaciones.")

    def test_acepta_el_prefijo_v_del_tag(self):
        self.assertEqual(seccion(EJEMPLO, "v0.2.0"), "- Actualizaciones.")

    def test_version_sin_seccion_falla(self):
        with self.assertRaises(ValueError):
            seccion(EJEMPLO, "0.3.0")

    def test_no_confunde_versiones_con_prefijo_comun(self):
        with self.assertRaises(ValueError):
            seccion(EJEMPLO, "0.2")

    def test_sin_publicar_no_es_una_version(self):
        with self.assertRaises(ValueError):
            seccion("## Sin publicar\n\n- Algo.\n", "0.2.2")

    def test_seccion_vacia_falla(self):
        with self.assertRaises(ValueError):
            seccion("## 0.2.2\n\n## 0.2.1\n\n- Algo.\n", "0.2.2")

    def test_finales_de_linea_de_windows(self):
        self.assertEqual(seccion("## 1.0.0\r\n\r\n- Algo.\r\n", "1.0.0"), "- Algo.")

    def test_el_changelog_del_repositorio_tiene_la_version_de_la_app(self):
        conf = os.path.join(RAIZ, "app", "src-tauri", "tauri.conf.json")
        with open(conf, encoding="utf-8") as f:
            version = json.load(f)["version"]
        with open(os.path.join(RAIZ, "CHANGELOG.md"), encoding="utf-8") as f:
            self.assertNotEqual(seccion(f.read(), version), "")


if __name__ == "__main__":
    unittest.main()
