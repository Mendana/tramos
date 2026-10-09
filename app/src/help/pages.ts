// Páginas de la ayuda (`docs/app.md`, "Ayuda"). Cada página es un .md de esta carpeta que se
// importa con `?raw`: Vite lo mete en el bundle, así que la ayuda funciona sin conexión, y si un
// fichero falta, `npm run build` falla al no poder resolver el import.
import ajustes from "./ajustes.md?raw";
import carrera from "./carrera.md?raw";
import carreraAnalisis from "./carrera-analisis.md?raw";
import carreraGrupo from "./carrera-grupo.md?raw";
import carreraMapa from "./carrera-mapa.md?raw";
import carreraTramos from "./carrera-tramos.md?raw";
import compartir from "./compartir.md?raw";
import estadisticas from "./estadisticas.md?raw";
import estadisticasCabeza from "./estadisticas-cabeza.md?raw";
import estadisticasDonde from "./estadisticas-donde.md?raw";
import estadisticasEvolucion from "./estadisticas-evolucion.md?raw";
import formatos from "./formatos.md?raw";
import grupo from "./grupo.md?raw";
import importar from "./importar.md?raw";
import indice from "./indice.md?raw";
import inicio from "./inicio.md?raw";
import ir from "./ir.md?raw";
import lentoODesorientado from "./lento-o-desorientado.md?raw";
import misCarreras from "./mis-carreras.md?raw";
import atletas from "./atletas.md?raw";
import perfil from "./perfil.md?raw";
import reloj from "./reloj.md?raw";
import tiempoPerdido from "./tiempo-perdido.md?raw";
import tiposDeError from "./tipos-de-error.md?raw";
import zonasDelMapa from "./zonas-del-mapa.md?raw";
import { markdownTitle } from "./markdown";

/** Identificador de una página: el nombre de su fichero sin `.md`. */
export type HelpPageId =
  | "indice"
  | "inicio"
  | "mis-carreras"
  | "importar"
  | "perfil"
  | "ajustes"
  | "grupo"
  | "carrera"
  | "carrera-tramos"
  | "carrera-mapa"
  | "carrera-grupo"
  | "carrera-analisis"
  | "estadisticas"
  | "estadisticas-donde"
  | "estadisticas-evolucion"
  | "estadisticas-cabeza"
  | "tiempo-perdido"
  | "ir"
  | "tipos-de-error"
  | "lento-o-desorientado"
  | "zonas-del-mapa"
  | "reloj"
  | "formatos"
  | "compartir"
  | "atletas";

type HelpGroup = "start" | "screens" | "race" | "history" | "concepts";

/** Bloques del índice, en orden. */
export const HELP_GROUPS: { id: HelpGroup; label: string }[] = [
  { id: "start", label: "Ayuda" },
  { id: "screens", label: "Pantallas" },
  { id: "race", label: "Una carrera" },
  { id: "history", label: "Estadísticas" },
  { id: "concepts", label: "Conceptos" },
];

interface HelpPage {
  source: string;
  group: HelpGroup;
}

/** Todas las páginas, en el orden del índice. */
export const HELP_PAGES: Record<HelpPageId, HelpPage> = {
  indice: { source: indice, group: "start" },
  inicio: { source: inicio, group: "screens" },
  "mis-carreras": { source: misCarreras, group: "screens" },
  importar: { source: importar, group: "screens" },
  perfil: { source: perfil, group: "screens" },
  ajustes: { source: ajustes, group: "screens" },
  grupo: { source: grupo, group: "screens" },
  carrera: { source: carrera, group: "race" },
  "carrera-tramos": { source: carreraTramos, group: "race" },
  "carrera-mapa": { source: carreraMapa, group: "race" },
  "carrera-grupo": { source: carreraGrupo, group: "race" },
  "carrera-analisis": { source: carreraAnalisis, group: "race" },
  estadisticas: { source: estadisticas, group: "history" },
  "estadisticas-donde": { source: estadisticasDonde, group: "history" },
  "estadisticas-evolucion": { source: estadisticasEvolucion, group: "history" },
  "estadisticas-cabeza": { source: estadisticasCabeza, group: "history" },
  "tiempo-perdido": { source: tiempoPerdido, group: "concepts" },
  ir: { source: ir, group: "concepts" },
  "tipos-de-error": { source: tiposDeError, group: "concepts" },
  "lento-o-desorientado": { source: lentoODesorientado, group: "concepts" },
  "zonas-del-mapa": { source: zonasDelMapa, group: "concepts" },
  reloj: { source: reloj, group: "concepts" },
  formatos: { source: formatos, group: "concepts" },
  compartir: { source: compartir, group: "concepts" },
  atletas: { source: atletas, group: "concepts" },
};

export function isHelpPageId(id: string): id is HelpPageId {
  return Object.prototype.hasOwnProperty.call(HELP_PAGES, id);
}

/** Título de una página: su `# …`. */
export function helpTitle(id: HelpPageId): string {
  return markdownTitle(HELP_PAGES[id].source) ?? "Ayuda";
}

/** Las páginas de un bloque del índice, en orden. */
export function helpPagesOf(group: HelpGroup): HelpPageId[] {
  return (Object.keys(HELP_PAGES) as HelpPageId[]).filter((id) => HELP_PAGES[id].group === group);
}
