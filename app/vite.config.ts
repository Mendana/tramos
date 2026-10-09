import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
// @ts-expect-error type error without @types/node package
import { existsSync, readdirSync, readFileSync } from "node:fs";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// Comprueba las páginas de ayuda al compilar (`npm run build`; `docs/app.md`, "Ayuda"): si algo
// no cuadra, el build falla con la lista de problemas.
//
// - Cada .md de `src/help/` está registrado en `src/help/pages.ts` y empieza por `# Título`.
// - No lleva HTML crudo (el renderizador no lo pinta y la CSP no deja estilos en línea) ni listas
//   anidadas (el renderizador solo entiende un nivel).
// - Sus enlaces van a otra página que existe (`otra.md`) o a un fichero del repositorio que
//   existe (`https://github.com/Mendana/tramos/blob/main/docs/...`). Ningún otro destino.
//
// Que cada pantalla y pestaña tenga su página lo comprueba `tsc` (`src/help/views.ts`).
const DOCS_BASE = "https://github.com/Mendana/tramos/blob/main/";
const HELP_DIR = new URL("./src/help/", import.meta.url);
const REPO_ROOT = new URL("../", import.meta.url);

function problemsOf(name: string, source: string, registry: string): string[] {
  const problems: string[] = [];
  const page = name.replace(/\.md$/, "");
  if (!/^[a-z0-9-]+$/.test(page)) problems.push("el nombre solo puede llevar a-z, 0-9 y guiones");
  if (!registry.includes(`"./${name}?raw"`)) problems.push("no está en src/help/pages.ts");
  if (!source.startsWith("# ")) problems.push("tiene que empezar por «# Título»");
  const withoutCode = source.replace(/`[^`]*`/g, "");
  if (/<\/?[a-zA-Z!]/.test(withoutCode)) problems.push("lleva HTML: usa solo Markdown");
  if (/^[ \t]+([-*]|\d+\.)\s/m.test(source))
    problems.push("lleva listas anidadas: solo hay un nivel");
  for (const match of withoutCode.matchAll(/\]\(([^)\s]*)\)/g)) {
    const href: string = match[1];
    const internal = /^([a-z0-9-]+\.md)$/.exec(href);
    if (internal !== null) {
      if (!existsSync(new URL(internal[1], HELP_DIR))) {
        problems.push(`enlaza a ${href}, que no existe`);
      }
    } else if (href.startsWith(DOCS_BASE)) {
      const path = href.slice(DOCS_BASE.length).replace(/#.*$/, "");
      if (path === "" || !existsSync(new URL(path, REPO_ROOT))) {
        problems.push(`enlaza a ${path} del repositorio, que no existe`);
      }
    } else {
      problems.push(`enlace no permitido: ${href} (solo otra-pagina.md o ${DOCS_BASE}...)`);
    }
  }
  return problems.map((p) => `src/help/${name}: ${p}`);
}

function helpCheck(): Plugin {
  return {
    name: "tramos-help-check",
    apply: "build",
    buildStart() {
      const registry: string = readFileSync(new URL("pages.ts", HELP_DIR), "utf8");
      const names = (readdirSync(HELP_DIR) as string[]).filter((f) => f.endsWith(".md")).sort();
      const problems = names.flatMap((name) =>
        problemsOf(name, readFileSync(new URL(name, HELP_DIR), "utf8"), registry),
      );
      if (problems.length > 0) {
        this.error(`Páginas de ayuda con problemas:\n${problems.join("\n")}`);
      }
    },
  };
}

// https://vite.dev/config/
export default defineConfig(() => ({
  // `helpCheck`: comprueba las páginas de ayuda al compilar (`docs/app.md`, "Ayuda").
  plugins: [react(), helpCheck()],

  // El worker de MapLibre (`MapView.tsx`) es un módulo ES: MapLibre lo crea con
  // `new Worker(url, { type: "module" })`.
  worker: {
    format: "es" as const,
  },
  // MapLibre (~1 MB minificado) va en su propio trozo, que solo se carga al abrir una carrera.
  build: {
    chunkSizeWarningLimit: 1100,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
