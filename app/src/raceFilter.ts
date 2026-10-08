// Búsqueda, filtros y orden de Mis carreras (#128, `docs/app.md`, "Lista de carreras"). Solo
// eligen y ordenan las filas que ya ha devuelto `list_races`: no calculan nada.
import { RaceFormat, RaceRow } from "./api";

export type RaceSort = "recent" | "lost" | "performance";

export const SORT_LABELS: Record<RaceSort, string> = {
  recent: "Más recientes",
  lost: "Más tiempo perdido",
  performance: "Mejor rendimiento",
};

export interface RaceFilter {
  /** Texto que buscar en el nombre de la carrera y la categoría. */
  query: string;
  /** `all` = todas; `none` = sin formato. */
  format: RaceFormat | "all" | "none";
  /** Año de la carrera; `null` = todos. */
  season: number | null;
  trackOnly: boolean;
  sort: RaceSort;
}

export const NO_FILTER: RaceFilter = {
  query: "",
  format: "all",
  season: null,
  trackOnly: false,
  sort: "recent",
};

/** Carreras por página. */
export const PAGE_SIZE = 25;

/** ¿Hay algún filtro puesto? (el orden no filtra). */
export function isFiltered(filter: RaceFilter): boolean {
  return (
    filter.query.trim() !== "" ||
    filter.format !== "all" ||
    filter.season !== null ||
    filter.trackOnly
  );
}

/** Sin mayúsculas ni tildes, para buscar «Cañada» con «canada». */
function fold(text: string): string {
  return text
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
}

/** Años con carreras, del más reciente al más antiguo. */
export function seasons(races: RaceRow[]): number[] {
  const years = new Set(races.map((r) => Number(r.date.slice(0, 4))));
  return [...years].sort((a, b) => b - a);
}

/** Valor para ordenar de mayor a menor; las carreras sin él van al final. */
function sortKey(race: RaceRow, sort: RaceSort): number | null {
  if (sort === "lost") return race.lost_time_s;
  if (sort === "performance") return race.usual_performance;
  return null;
}

/** Las carreras que pasan el filtro, en el orden pedido. `races` viene de la más reciente. */
export function filterRaces(races: RaceRow[], filter: RaceFilter): RaceRow[] {
  const words = fold(filter.query)
    .split(/\s+/)
    .filter((w) => w !== "");
  const rows = races.filter((race) => {
    if (
      filter.format === "none"
        ? race.format !== null
        : filter.format !== "all" && race.format !== filter.format
    ) {
      return false;
    }
    if (filter.season !== null && Number(race.date.slice(0, 4)) !== filter.season) return false;
    if (filter.trackOnly && !race.has_track) return false;
    const text = fold(`${race.name ?? ""} ${race.class_name}`);
    return words.every((w) => text.includes(w));
  });
  if (filter.sort === "recent") return rows;
  // `sort` es estable: a igualdad, se queda la más reciente delante.
  return [...rows].sort((a, b) => {
    const ka = sortKey(a, filter.sort);
    const kb = sortKey(b, filter.sort);
    if (ka === null || kb === null) return ka === null ? (kb === null ? 0 : 1) : -1;
    return kb - ka;
  });
}
