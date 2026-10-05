// Tipos y llamadas a los comandos de `src-tauri/src/lib.rs`. Los campos van en snake_case, como
// los serializa Rust; solo los nombres de los argumentos de primer nivel pasan a camelCase.
import { invoke } from "@tauri-apps/api/core";

export type RaceFormat = "sprint" | "middle" | "long";
export type RaceStatus =
  | "ok"
  | "not_classified"
  | "did_not_start"
  | { unknown: number };

export interface RunnerIdentity {
  si_card: number | null;
  full_name: string | null;
}

export interface ResultRef {
  class_index: number;
  result_index: number;
}

export interface RunnerChoice {
  result: ResultRef;
  class_name: string;
  given_name: string;
  family_name: string;
  club: string | null;
  si_card: number | null;
  status: RaceStatus;
  place: number | null;
}

export type Match = "unique" | "unique_name_mismatch" | "ambiguous" | "not_found";

export interface ImportPreview {
  event_name: string | null;
  event_date: string;
  suggested_format: RaceFormat | null;
  median_winner_s: number | null;
  matched: Match;
  candidates: RunnerChoice[];
  already_imported: boolean;
  fit_points: number | null;
}

export interface ImportRequest {
  spl_path: string;
  fit_path: string | null;
  result: ResultRef;
  format: RaceFormat | null;
  identity: RunnerIdentity;
}

export interface AlignmentReport {
  track_saved: boolean;
  offset_s: number | null;
  confidence: number | null;
  messages: string[];
}

export interface ImportOutcome {
  event_id: number;
  result_id: number;
  already_imported: boolean;
  alignment: AlignmentReport | null;
  warnings: string[];
}

/** Qué se ha hecho con un .spl de la carpeta (`docs/app.md`, "Importar una carpeta"). */
export type BatchRaceStatus = "imported" | "already_imported" | "not_imported";

export interface PairedFit {
  path: string;
  track_saved: boolean;
  offset_s: number | null;
  confidence: number | null;
}

export interface BatchRace {
  spl_path: string;
  /** `null` si el .spl no se ha podido leer. */
  event_name: string | null;
  event_date: string | null;
  status: BatchRaceStatus;
  result_id: number | null;
  fit: PairedFit | null;
  /** Avisos o el motivo de no importarla. */
  messages: string[];
}

export type UnpairedReason = "no_race" | "invalid" | "duplicate";

export interface UnpairedFit {
  path: string;
  reason: UnpairedReason;
  message: string;
}

export interface BatchSummary {
  /** De la carrera más antigua a la más reciente; los .spl ilegibles, al final. */
  races: BatchRace[];
  unpaired_fits: UnpairedFit[];
  warnings: string[];
}

export interface RaceRow {
  event_id: number;
  result_id: number;
  date: string;
  name: string | null;
  class_name: string;
  status: RaceStatus;
  place: number | null;
  format: RaceFormat | null;
  has_track: boolean;
  total_s: number | null;
  lost_time_s: number | null;
  error_count: number;
}

export interface LostTimeConfig {
  error_threshold_s: number;
  error_threshold_pct: number;
  ideal_time: "sum_of_references" | "sum_of_best_splits";
}

export interface LegReport {
  index: number;
  from: number;
  to: number;
  split_s: number | null;
  elapsed_s: number | null;
  place: number | null;
  reference_s: number | null;
  reference_count: number;
  valid_splits: number;
  performance_index: number | null;
  expected_s: number | null;
  loss_s: number | null;
  loss_pct: number | null;
  is_error: boolean;
  /** `expected_s − split_s` (P5): positiva si el tramo fue mejor de lo esperado. */
  gain_s: number | null;
  cumulative_gain_s: number | null;
  ideal_elapsed_s: number | null;
  behind_ideal_s: number | null;
  is_last: boolean;
  short_reference: boolean;
  excluded_from_patterns: boolean;
}

/** Dos o más tramos seguidos perdiendo (P5). Tramos numerados desde 1. */
export interface LosingStreak {
  first_leg: number;
  last_leg: number;
  loss_s: number;
}

export interface RunnerReport {
  course: {
    controls: number[];
    classes: { index: number; id: number; name: string }[];
    valid_runners: number;
    weak_reference: boolean;
  };
  lost_time: {
    total_s: number | null;
    usual_performance: number | null;
    lost_time_s: number | null;
    error_count: number;
    time_without_errors_s: number | null;
    ideal_time_s: number | null;
    behind_ideal_s: number | null;
    losing_streaks: LosingStreak[];
    /** Consistencia (P10): desviación típica del IR por tramo, ponderada por la referencia (1 =
     * 100 puntos). `null` con menos de dos tramos que cuenten. */
    consistency: number | null;
    legs: LegReport[];
  };
}

export interface RaceDetail {
  event_id: number;
  result_id: number;
  date: string;
  name: string | null;
  format: RaceFormat | null;
  class_name: string;
  given_name: string;
  family_name: string;
  club: string | null;
  si_card: number | null;
  status: RaceStatus;
  place: number | null;
  config: LostTimeConfig;
  report: RunnerReport;
}

/** Posición de un resultado en la carrera (como en el núcleo). */
export interface ResultRef {
  class_index: number;
  result_index: number;
}

/** Un corredor del recorrido, con sus series por tramo (P4). */
export interface ComparedRunner {
  result: ResultRef;
  is_self: boolean;
  given_name: string;
  family_name: string;
  club: string | null;
  class_name: string;
  status: RaceStatus;
  place: number | null;
  /** Puesto juntando todas las categorías del recorrido. */
  course_place: number | null;
  total_s: number | null;
  behind_ideal_s: (number | null)[];
  loss_s: (number | null)[];
  is_error: boolean[];
}

export interface CourseComparison {
  legs: { index: number; from: number; to: number }[];
  ideal_time_s: number | null;
  /** Clasificados por tiempo y después el resto. */
  runners: ComparedRunner[];
}

/** Qué carreras entran en el histórico. `null` = sin filtro; fechas `AAAA-MM-DD`, incluidas. */
export interface HistoryFilter {
  from: string | null;
  to: string | null;
  format: RaceFormat | null;
}

/** Números agregados de un grupo de carreras (`docs/historico.md`). */
export interface HistoryStats {
  /** Carreras con números. */
  races: number;
  /** Tramos que cuentan: con pérdida, sin el último ni los de referencia corta. */
  legs: number;
  errors: number;
  /** Media del rendimiento habitual de cada carrera (1 = 100 %). */
  mean_performance: number | null;
  /** Errores / tramos (0–1). */
  error_rate: number | null;
  /** Pérdida de los errores repartida entre los tramos (s). */
  mean_loss_s: number | null;
  /** Lo mismo en % del tiempo esperado. */
  mean_loss_pct: number | null;
  /** Media de la consistencia de las carreras que la tienen (P10, 1 = 100 puntos). */
  mean_consistency: number | null;
}

export interface FormatHistory {
  /** `null` = carreras sin formato. */
  format: RaceFormat | null;
  stats: HistoryStats;
}

/** Una carrera del histórico con sus números (#98). */
export interface HistoryRaceRow {
  result_id: number;
  date: string;
  name: string | null;
  format: RaceFormat | null;
  class_name: string;
  status: RaceStatus;
  place: number | null;
  /** `null` sin rendimiento habitual: no cuenta. */
  stats: HistoryStats | null;
}

export interface HistoryView {
  /** Carreras del usuario sin filtrar. */
  all_races: number;
  first_date: string | null;
  last_date: string | null;
  config: LostTimeConfig;
  history: {
    filter: HistoryFilter;
    /** Sprint, media y larga (o solo el formato del filtro) y, si hay, las sin formato. */
    by_format: FormatHistory[];
    total: HistoryStats;
    races_without_data: number;
  };
  /** Las carreras que pasan el filtro, de la más reciente a la más antigua. */
  races: HistoryRaceRow[];
  /** Pérdida según duración del tramo (P7): los seis cubos, con el mismo filtro. */
  by_leg_length: LegLengthStats[];
  /** Días sin competir (P11): cubos por días desde la carrera anterior, con el mismo filtro. */
  days_off: {
    buckets: DaysOffStats[];
    /** Carreras que pasan el filtro sin ninguna anterior. */
    without_previous: number;
  };
  /** Pérdida según desnivel (P13): subida, llano y bajada, con el mismo filtro. */
  by_slope: SlopeHistory;
  /** ¿Lento o desorientado? (P2): la pérdida de los errores repartida, con el mismo filtro. */
  loss_breakdown: BreakdownHistory;
  /** Después de fallar (P8), con el mismo filtro. */
  after_error: AfterError;
  /** Errores más comunes (P9): por tipo, en total, por formato y por duración del tramo. */
  common_errors: CommonErrors;
  /** ¿El cansancio anticipa el error? (P14): por tercio de carrera, con el mismo filtro. */
  fatigue: Fatigue;
}

/** Un cubo de días desde la carrera anterior (P11, `docs/historico.md`), ambos extremos incluidos. */
export interface DaysOffStats {
  from_days: number;
  /** `null` en el último cubo (sin final). */
  to_days: number | null;
  races: number;
  /** Tramos con IR entre los tres primeros de esas carreras. */
  first_legs: number;
  /** Media de su IR (1 = 100 %). */
  first_legs_performance: number | null;
  /** Tramos que cuentan del primer tercio. */
  first_third_legs: number;
  first_third_errors: number;
  /** Errores / tramos del primer tercio (0–1). */
  first_third_error_rate: number | null;
}

/** Clase de desnivel de un tramo (P13, `docs/historico.md`). */
export type SlopeClass = "uphill" | "flat" | "downhill";

export const SLOPE_LABELS: Record<SlopeClass, string> = {
  uphill: "Subida",
  flat: "Llano",
  downhill: "Bajada",
};

/** Números de una clase de desnivel (P13). */
export interface SlopeStats {
  class: SlopeClass;
  /** Tramos que cuentan de la clase (n). */
  legs: number;
  errors: number;
  /** Errores / tramos (0–1). */
  error_rate: number | null;
  /** IR medio de los tramos de la clase, ponderado por la referencia (1 = 100 %). */
  mean_performance: number | null;
}

/** P13 con los filtros del histórico: las tres clases y de dónde salen los tramos. */
export interface SlopeHistory {
  config: { threshold_m_per_100m: number };
  /** Subida, llano y bajada, siempre y en ese orden. */
  by_class: SlopeStats[];
  /** Carreras con números y track: las que aportan tramos. */
  races_with_track: number;
  /** Carreras con números sin track: no aportan tramos. */
  races_without_track: number;
  /** Tramos que cuentan de las carreras sin track. */
  legs_without_track: number;
  /** Tramos que cuentan de carreras con track que no se pueden clasificar. */
  unclassified_legs: number;
}

/** Un cubo de duración de tramo (P7, `docs/historico.md`): referencia en `[from_s, to_s)`. */
export interface LegLengthStats {
  from_s: number;
  /** `null` en el último cubo (sin final). */
  to_s: number | null;
  /** Tramos que cuentan con la referencia en el cubo (n). */
  legs: number;
  errors: number;
  /** Errores / tramos (0–1). */
  error_rate: number | null;
  /** Pérdida de los errores repartida entre los tramos (s). */
  mean_loss_s: number | null;
  /** Lo mismo en % del tiempo esperado. */
  mean_loss_pct: number | null;
}

export const FORMAT_LABELS: Record<RaceFormat, string> = {
  sprint: "Sprint",
  middle: "Media",
  long: "Larga",
};

/** Nombre del fichero de una ruta (con `/` o `\\`). */
export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export function statusLabel(status: RaceStatus, place: number | null): string {
  if (status === "ok") return place === null ? "Clasificado" : `${place}.º`;
  if (status === "not_classified") return "No clasificado";
  if (status === "did_not_start") return "No presentado";
  return `Estado ${status.unknown}`;
}

export const coreVersion = () => invoke<string>("core_version");

export interface Settings {
  error_threshold_s: number;
  error_threshold_pct: number;
  time_zone: string;
  identity: RunnerIdentity;
}

export const getSettings = () => invoke<Settings>("get_settings");

export const saveSettings = (settings: Settings) =>
  invoke<void>("save_settings", { settings });

export const previewImport = (
  splPath: string,
  fitPath: string | null,
  identity: RunnerIdentity,
) => invoke<ImportPreview>("preview_import", { splPath, fitPath, identity });

export const importRace = (request: ImportRequest) =>
  invoke<ImportOutcome>("import_race", { request });

export const importFolder = (folderPath: string) =>
  invoke<BatchSummary>("import_folder", { folderPath });

export const listRaces = () => invoke<RaceRow[]>("list_races");

export const raceDetail = (resultId: number) =>
  invoke<RaceDetail>("race_detail", { resultId });

export const setRaceFormat = (resultId: number, format: RaceFormat | null) =>
  invoke<void>("set_race_format", { resultId, format });

export const raceComparison = (resultId: number) =>
  invoke<CourseComparison>("race_comparison", { resultId });

/**
 * Desfase entre el reloj y el cronometraje (#68, `clock_offset.rs`). Convenio: instante en el
 * track = picada + desfase; un reloj adelantado da un desfase positivo.
 */
export interface OffsetView {
  /** El que calcula la alineación; `null` si no se puede alinear (`automatic_error`). */
  automatic_offset_s: number | null;
  /** `false` si no había picadas útiles y el automático es 0 por defecto. */
  automatic_estimated: boolean;
  /** 0–1. */
  confidence: number | null;
  low_confidence: boolean;
  automatic_warnings: string[];
  automatic_error: string | null;
  /** ±3600 o ±7200 si la hora parece mal convertida. */
  suggested_shift_s: number | null;
  /** El desplazamiento sugerido más el desfase fino que queda con él. */
  suggested_offset_s: number | null;
  /** Fijado a mano; `null` = automático. */
  manual_offset_s: number | null;
  max_manual_offset_s: number;
}

export const raceOffset = (resultId: number) =>
  invoke<OffsetView | null>("race_offset", { resultId });

/** `null` vuelve al automático. */
export const setRaceOffset = (resultId: number, offsetS: number | null) =>
  invoke<OffsetView>("set_race_offset", { resultId, offsetS });

/** Reparto de la pérdida de un tramo (P2, `docs/tiempo-perdido.md`): pérdida = desvío + paradas + ritmo. */
export interface LegBreakdown {
  index: number;
  loss_s: number;
  is_error: boolean;
  detour_s: number;
  stopped_s: number;
  pace_s: number;
}

/** Sumas del reparto de varios tramos. */
export interface BreakdownTotals {
  legs: number;
  loss_s: number;
  detour_s: number;
  stopped_s: number;
  pace_s: number;
}

/** ¿Lento o desorientado? (P2) de una carrera. */
export interface RaceBreakdown {
  /** Relación habitual distancia / línea recta (r0); `null` con pocos tramos sin error. */
  usual_ratio: number | null;
  clean_legs: number;
  /** Tramos que cuentan con reparto. */
  legs: LegBreakdown[];
  /** Suma de los errores de `legs`. */
  errors: BreakdownTotals;
  errors_without_breakdown: number;
}

/** P2 en el histórico. */
export interface BreakdownHistory {
  errors: BreakdownTotals;
  races_with_track: number;
  races_without_track: number;
  errors_without_breakdown: number;
}

/** Tramos y errores de un grupo de tramos (P8). */
export interface Rate {
  legs: number;
  errors: number;
  /** Errores / tramos (0–1). */
  error_rate: number | null;
}

/** Después de fallar (P8, `docs/historico.md`). */
export interface AfterError {
  after_error: Rate;
  after_clean: Rate;
  /** Tras un error, yendo > 5 % más rápido que la mediana de los tramos limpios de la carrera. */
  accelerated: Rate;
  not_accelerated: Rate;
  after_error_without_speed: number;
  /** Tramos limpios seguidos justo antes: de `from` a `to` (`null` = sin fin). */
  streaks: { from: number; to: number | null; rate: Rate }[];
}

/** Deriva de un tercio (P14): pulso frente a velocidad en los tramos limpios. */
export interface Drift {
  /** Tramos limpios con pulso y velocidad (n). */
  legs: number;
  mean_heart_rate_bpm: number | null;
  mean_speed_mps: number | null;
  /** Pulso / velocidad frente a la mediana de los tramos limpios de su carrera (1 = lo habitual). */
  mean_relative_ratio: number | null;
}

/** Pulso del tramo anterior de un grupo de tramos (P14). */
export interface HeartRateBefore {
  /** Tramos cuyo anterior tiene pulso (n). */
  legs: number;
  mean_heart_rate_bpm: number | null;
  /** Pulso del anterior menos la mediana de pulso de su carrera (ppm). */
  mean_relative_bpm: number | null;
}

/** Esfuerzo percibido (1–10) apuntado en las etiquetas. */
export interface Effort {
  legs: number;
  mean_effort: number | null;
}

/** P14 en un tercio de carrera. */
export interface ThirdFatigue {
  /** 1, 2 o 3. */
  third: number;
  drift: Drift;
  before_error: HeartRateBefore;
  before_clean: HeartRateBefore;
  effort_error: Effort;
  effort_clean: Effort;
  effort_physical: Effort;
}

/** ¿El cansancio anticipa el error? (P14, `docs/historico.md`). */
export interface Fatigue {
  /** Los tres tercios, en orden. */
  by_third: ThirdFatigue[];
  races_with_heart_rate: number;
  races_without_heart_rate: number;
  /** Errores sin pulso del tramo anterior para comparar. */
  errors_without_heart_rate: number;
}

/** `null` si la carrera no tiene el FIT del reloj. */
export const raceBreakdown = (resultId: number) =>
  invoke<RaceBreakdown | null>("race_breakdown", { resultId });

export const getHistory = (filter: HistoryFilter) =>
  invoke<HistoryView>("history", { filter });

// Etiquetado de errores (docs/taxonomia.md).
export interface TaxonomyEntry {
  key: string;
  label: string;
}

export interface ErrorType extends TaxonomyEntry {
  description?: string;
  subtypes: TaxonomyEntry[];
}

export interface Taxonomy {
  version: string;
  types: ErrorType[];
  causes: TaxonomyEntry[];
}

export type Confirmation = "error" | "no_error" | "physical";
export type LegPart = "start" | "middle" | "attack";

export const CONFIRMATION_LABELS: Record<Confirmation, string> = {
  error: "Sí",
  no_error: "No",
  physical: "Físico",
};

export const LEG_PART_LABELS: Record<LegPart, string> = {
  start: "Salida",
  middle: "Mitad",
  attack: "Ataque",
};

/** Etiqueta de un tramo; todo es opcional. */
export interface LegTag {
  confirmation: Confirmation | null;
  error_type: string | null;
  error_subtype: string | null;
  causes: string[];
  leg_part: LegPart | null;
  perceived_loss_s: number | null;
  effort: number | null;
  note: string | null;
}

export const EMPTY_TAG: LegTag = {
  confirmation: null,
  error_type: null,
  error_subtype: null,
  causes: [],
  leg_part: null,
  perceived_loss_s: null,
  effort: null,
  note: null,
};

export interface TagView {
  leg_index: number;
  taxonomy_version: string;
  tag: LegTag;
  created_at: string;
  updated_at: string;
}

// Errores más comunes (P9).
export interface SubtypeCount {
  subtype: string | null;
  errors: number;
  loss_s: number;
}

export interface TypeCount {
  error_type: string;
  errors: number;
  loss_s: number;
  subtypes: SubtypeCount[];
}

export interface ErrorTypes {
  legs: number;
  errors: number;
  loss_s: number;
  untyped: number;
  untyped_loss_s: number;
  unreviewed: number;
  by_type: TypeCount[];
  physical_legs: number;
  physical_loss_s: number;
}

export interface LengthErrorTypes {
  from_s: number;
  to_s: number | null;
  types: ErrorTypes;
}

export interface CommonErrors {
  total: ErrorTypes;
  by_leg_length: LengthErrorTypes[];
  by_format: { format: RaceFormat | null; total: ErrorTypes; by_leg_length: LengthErrorTypes[] }[];
}

export const getTaxonomy = () => invoke<Taxonomy>("taxonomy");

export const legTags = (resultId: number) => invoke<TagView[]>("leg_tags", { resultId });

/** Guarda la etiqueta de un tramo; vacía, la borra y devuelve `null`. */
export const saveLegTag = (resultId: number, legIndex: number, tag: LegTag) =>
  invoke<TagView | null>("save_leg_tag", { resultId, legIndex, tag });

/** Duración redondeada al segundo: `m:ss`, o `h:mm:ss` desde una hora (como la CLI). */
export function clock(seconds: number | null): string {
  if (seconds === null) return "—";
  const total = Math.round(Math.abs(seconds));
  const sign = seconds < 0 && total > 0 ? "-" : "";
  const h = Math.floor(total / 3600);
  const m = Math.floor(total / 60) % 60;
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${sign}${h}:${String(m).padStart(2, "0")}:${s}` : `${sign}${m}:${s}`;
}

/** Número con coma decimal. */
export function decimal(value: number, decimals: number): string {
  return value.toFixed(decimals).replace(".", ",");
}

/** Consistencia (P10) en puntos de IR: `± 8,3 %`. Menor = más consistente. */
export function spread(consistency: number | null, decimals = 1): string {
  return consistency === null ? "—" : `± ${decimal(consistency * 100, decimals)} %`;
}

/** Con signo y un decimal: `+5,3`, `-2,0`. */
export function signed(value: number): string {
  return (value >= 0 ? "+" : "") + decimal(value, 1);
}

/** Código de baliza con la salida y la meta como S y M. */
export function codeLabel(code: number): string {
  if (code === 32736) return "S";
  if (code === 32752) return "M";
  return String(code);
}
