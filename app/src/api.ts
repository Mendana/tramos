// Tipos y llamadas a los comandos de `src-tauri/src/lib.rs`. Los campos van en snake_case, como
// los serializa Rust; solo los nombres de los argumentos de primer nivel pasan a camelCase.
import { invoke } from "@tauri-apps/api/core";

export type RaceFormat = "sprint" | "middle" | "long";
export type RaceStatus = "ok" | "not_classified" | "did_not_start" | { unknown: number };

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
  /** Tramos con error aún sin confirmar (Sí, No o Físico). */
  unreviewed_count: number;
  /** Rendimiento habitual (1 = 100 %). */
  usual_performance: number | null;
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
  /** Resumen en frases de la carrera: como mucho tres (#126). */
  insights: Insight<RaceInsightTarget>[];
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

/** Regla que ha dado una frase del resumen (#126, `docs/frases.md`). */
export type InsightRule =
  | "leg_length"
  | "common_error"
  | "after_error"
  | "loss_breakdown"
  | "slope"
  | "format"
  | "days_off"
  | "clean_race"
  | "concentrated_loss"
  | "losing_streak"
  | "errors_by_third";

/** Análisis del histórico que justifica una frase: a dónde lleva su enlace. */
export type HistoryInsightTarget =
  | "formats"
  | "leg_length"
  | "common_errors"
  | "slope"
  | "loss_breakdown"
  | "after_error"
  | "days_off";

/** Parte de la carrera que justifica una frase. */
export type RaceInsightTarget = "legs" | "gain_loss";

/**
 * Una frase del resumen (#126, `docs/frases.md`). La escribe el núcleo con sus números; la app
 * solo la enseña y enlaza su destino.
 */
export interface Insight<T extends string> {
  rule: InsightRule;
  text: string;
  /** Sale con pocos datos: va con el aviso `caveat`. */
  few_data: boolean;
  /** «con pocas carreras», «con pocos tramos»…; `null` con datos suficientes. */
  caveat: string | null;
  target: T;
}

export interface HistoryView {
  /** Carreras del usuario sin filtrar. */
  all_races: number;
  first_date: string | null;
  last_date: string | null;
  config: LostTimeConfig;
  /** Resumen en frases: como mucho tres, con el mismo filtro (#126). */
  insights: Insight<HistoryInsightTarget>[];
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
  /** Suma de las referencias de esos tramos (s): el peso del IR medio. */
  reference_s: number;
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
  // Otros códigos del .spl (5, 7…, #55): no clasificado, con el código original.
  return `No clasificado (código ${status.unknown})`;
}

export const coreVersion = () => invoke<string>("core_version");

export interface Settings {
  error_threshold_s: number;
  error_threshold_pct: number;
  time_zone: string;
  identity: RunnerIdentity;
  sharing: SharingSettings;
  map: MapSettings;
}

/** Zonas de color del mapa (#96, `src-tauri/src/zones.rs`), de menor a mayor valor. */
export interface Zones {
  /** Límites entre zonas, de menor a mayor (uno menos que colores). Un valor justo en un límite
   *  va a la zona de arriba. */
  limits: number[];
  /** `#rrggbb`. */
  colors: string[];
}

/** Colores del track: zonas propias o, con `null`, clases por cuantiles de cada carrera. */
export interface MapSettings {
  /** En s/km: la primera zona es la más rápida. */
  pace_zones: Zones | null;
  /** En ppm. */
  heart_rate_zones: Zones | null;
}

/** Rango de la zona `k` («< 120», «120–140», «≥ 160»), con los límites escritos con `format`. */
export function zoneLabel(limits: number[], k: number, format: (v: number) => string): string {
  if (limits.length === 0) return "Todo";
  if (k === 0) return `< ${format(limits[0])}`;
  if (k >= limits.length) return `≥ ${format(limits[limits.length - 1])}`;
  return `${format(limits[k - 1])}–${format(limits[k])}`;
}

/** Zonas como mucho (`MAX_ZONES` en `zones.rs`). */
export const MAX_ZONES = 10;

/** Colores que se proponen para cada zona, en orden: se ven sobre el mapa y se distinguen de
 *  la zona de al lado (lo comprueba un test de `zones.rs`). */
export const ZONE_COLORS = [
  "#6b7280",
  "#2563eb",
  "#15803d",
  "#d97706",
  "#b91c1c",
  "#7e22ce",
  "#0e7490",
  "#a16207",
  "#be185d",
  "#1e293b",
];

/** Cómo se usa la app, en la bienvenida: corre, entrena o las dos cosas (#119). */
export type Role = "runner" | "coach" | "both";

/** Qué se comparte de una carrera con quien te entrena (`docs/paquete.md`). */
export type ShareChoice = "none" | "aggregates" | "legs" | "track";

/** Nivel de un paquete: lo que se comparte, sin «nada». */
export type ShareLevel = Exclude<ShareChoice, "none">;

export const SHARE_LABELS: Record<ShareChoice, string> = {
  none: "Nada",
  aggregates: "Resumen",
  legs: "Tramos",
  track: "Track completo",
};

export const SHARE_HINTS: Record<ShareChoice, string> = {
  none: "No se comparte.",
  aggregates: "Tiempo, puesto, tiempo perdido y errores de la carrera.",
  legs: "Además, los tramos y tus etiquetas. Sin pulso ni GPS.",
  track: "Además, el track del reloj: GPS y pulso.",
};

export interface SharingSettings {
  /** Exporta las carreras propias a la carpeta («Compartir mis carreras»). */
  share_own: boolean;
  /** Entrena a otros atletas: recibe sus paquetes y activa la sección Atletas. */
  coach: boolean;
  /** Carpeta compartida; `null` = sin compartir. */
  folder: string | null;
  default_choice: ShareChoice;
}

/** Cómo queda una carrera en la carpeta compartida. */
export interface RaceSharing {
  /** Se puede compartir: compartes lo tuyo, con carpeta y la carrera es tuya. */
  available: boolean;
  /** Lo elegido para esta carrera; `null` = lo de por defecto. */
  choice: ShareChoice | null;
  default_choice: ShareChoice;
  /** Con qué nivel está en la carpeta; `null` = no está. */
  shared: ShareLevel | null;
  problem: string | null;
}

export interface ShareReport {
  written: number;
  unchanged: number;
  not_shared: number;
  problems: string[];
}

export interface ReceiveReport {
  created: number;
  replaced: number;
  unchanged: number;
  problems: string[];
  packages: number;
  runners: number;
}

export const raceSharing = (resultId: number) => invoke<RaceSharing>("race_sharing", { resultId });

export const setRaceSharing = (resultId: number, choice: ShareChoice | null) =>
  invoke<RaceSharing>("set_race_sharing", { resultId, choice });

export const shareAll = () => invoke<ShareReport>("share_all");

export const receivePackages = () => invoke<ReceiveReport>("receive_packages");

/** Resumen de una carrera tal y como lo vio el corredor (`docs/paquete.md`). */
export interface RaceSummary {
  class_name: string;
  status: RaceStatus;
  place: number | null;
  total_s: number | null;
  lost_time_s: number | null;
  error_count: number;
  time_without_errors_s: number | null;
  usual_performance: number | null;
  consistency: number | null;
}

/** Un atleta en el selector de la sección Atletas. */
export interface CoachRunner {
  runner_id: string;
  display_name: string;
  races: number;
  last_exported_at: string;
}

/** Carrera compartida solo con el resumen: no se puede abrir. */
export interface SummaryRace {
  date: string;
  name: string | null;
  format: RaceFormat | null;
  summary: RaceSummary;
}

/** Lo que no sale en las vistas de corredor del corredor que se ve. */
export interface RunnerViewInfo {
  runner: CoachRunner;
  summary_only: SummaryRace[];
  problems: string[];
}

export const roleChosen = () => invoke<boolean>("role_chosen");

export const chooseRole = (role: Role) => invoke<void>("choose_role", { role });

export const coachRunners = () => invoke<CoachRunner[]>("coach_runners");

/** Elige el atleta que se ve, en solo lectura (`null` = volver a lo propio). */
export const viewRunner = (runnerId: string | null) =>
  invoke<RunnerViewInfo | null>("view_runner", { runnerId });

/** El tipo de error más común de un corredor (P15). */
export interface TopError {
  error_type: string;
  errors: number;
  /** Parte de sus errores de orientación (0–1). */
  share: number;
}

/** Lo principal del histórico de un corredor en la tabla del grupo. */
export interface GroupRow {
  stats: HistoryStats;
  top_error: TopError | null;
  /** Su cubo de duración con más tasa de error, entre los que tienen bastantes tramos. */
  weakest_leg_length: LegLengthStats | null;
  slope: SlopeStats[];
}

export interface GroupRunnerRow {
  runner: CoachRunner;
  /** Las carreras propias de quien usa la app («Incluirme»). */
  is_self: boolean;
  row: GroupRow | null;
  problem: string | null;
}

export interface SharedResult {
  /** Posición del corredor en `GroupView.runners`. */
  runner: number;
  stats: HistoryStats;
}

export interface SharedRace {
  race_id: string;
  date: string;
  name: string | null;
  format: RaceFormat | null;
  /** De más a menos IR. */
  results: SharedResult[];
}

export interface HeadToHead {
  runner: number;
  other: number;
  races: number;
  better: number;
  worse: number;
  /** Media de IR(runner) − IR(other) (1 = 100 puntos). */
  mean_difference: number;
}

/** Vista de grupo (P15). */
export interface GroupView {
  runners: GroupRunnerRow[];
  comparison: { shared_races: SharedRace[]; head_to_head: HeadToHead[] };
  /** Totales de todos los que salen: sus carreras y tramos juntos (#120). */
  total: HistoryStats;
  /** Si las carreras propias cuentan en «Todos» (ajuste «Incluirme»). */
  include_self: boolean;
  /** El grupo de atletas al que se limita; `null` = todos. */
  group: number | null;
}

/** La vista de grupo; con `group`, solo los miembros de ese grupo de atletas. */
export const groupView = (filter: HistoryFilter, group: number | null = null) =>
  invoke<GroupView>("group_view", { filter, group });

/** Colores para los grupos de atletas, en este orden (el nuevo grupo toma el primero libre). */
export const GROUP_COLORS = [
  "#2563eb",
  "#d97706",
  "#15803d",
  "#be185d",
  "#7e22ce",
  "#0e7490",
  "#b91c1c",
  "#6b7280",
];

/** Lo que se edita de un grupo de atletas. */
export interface GroupFields {
  name: string;
  description: string;
  /** `#rrggbb`. */
  color: string;
}

export interface GroupInfo extends GroupFields {
  id: number;
  /** `runner_id` de sus miembros. */
  members: string[];
}

/** Alguien a quien se puede meter en un grupo. */
export interface GroupCandidate {
  runner_id: string;
  display_name: string;
  /** Quien usa la app, con su propio identificador. */
  is_self: boolean;
}

export interface GroupsView {
  groups: GroupInfo[];
  candidates: GroupCandidate[];
}

export const athleteGroups = () => invoke<GroupsView>("athlete_groups");

export const createAthleteGroup = (group: GroupFields) =>
  invoke<number>("create_athlete_group", { group });

export const updateAthleteGroup = (id: number, group: GroupFields) =>
  invoke<void>("update_athlete_group", { id, group });

export const deleteAthleteGroup = (id: number) => invoke<void>("delete_athlete_group", { id });

/** Qué se hace con un atleta que está en los dos grupos al compararlos (#121). */
export type Overlap = "count_in_both" | "exclude";

/** Qué carreras entran al comparar dos grupos. */
export type RaceSelection = "shared" | "all";

export interface CompareOptions {
  overlap: Overlap;
  races: RaceSelection;
}

/** Un tipo de error en un grupo. */
export interface TypeShare {
  error_type: string;
  errors: number;
  /** Parte de los errores de orientación del grupo (0–1), contando los sin tipo. */
  share: number;
}

/** Un grupo, con sus miembros juntos. */
export interface GroupSide {
  /** Miembros con alguna carrera que cuenta. */
  runners: number;
  stats: HistoryStats;
  by_leg_length: LegLengthStats[];
  by_slope: SlopeStats[];
  orientation_errors: number;
  untyped: number;
  /** De más a menos errores. */
  by_type: TypeShare[];
}

/** Un grupo frente a otro (`docs/historico.md`, "Comparar grupos"). */
export interface GroupsComparison {
  options: CompareOptions;
  a: GroupSide;
  b: GroupSide;
  /** Atletas que están en los dos grupos. */
  in_both: number;
  /** Con «solo las de los dos», cuántas carreras entran; con «todas», `null`. */
  shared_races: number | null;
  /** A − B (1 = 100 puntos). */
  performance_difference: number | null;
  /** A − B (0–1). */
  error_rate_difference: number | null;
  /** A − B (%). */
  loss_pct_difference: number | null;
}

export const compareAthleteGroups = (
  filter: HistoryFilter,
  a: number,
  b: number,
  options: CompareOptions,
) => invoke<GroupsComparison>("compare_athlete_groups", { filter, a, b, options });

/** Los análisis de Estadísticas con todos los miembros de un grupo juntos (#145). */
export interface GroupStats {
  /** Miembros con alguna carrera que cuenta. */
  runners: number;
  /** Por formato y total; la consistencia no se junta y va siempre a `null`. */
  history: HistoryView["history"];
  by_leg_length: LegLengthStats[];
  by_slope: SlopeHistory;
  loss_breakdown: BreakdownHistory;
  after_error: AfterError;
  common_errors: CommonErrors;
  fatigue: Fatigue;
  days_off: HistoryView["days_off"];
}

export interface GroupStatsMember {
  runner: CoachRunner;
  is_self: boolean;
  /** Sus carreras que cuentan con el filtro. */
  races: number;
  /** Si su histórico no se ha podido calcular, por qué: no cuenta. */
  problem: string | null;
}

/** Estadísticas de un grupo de atletas (`docs/historico.md`, "Estadísticas de un grupo"). */
export interface GroupStatsView extends GroupFields {
  id: number;
  members: GroupStatsMember[];
  /** Miembros de los que ya no hay carreras: no cuentan. */
  missing: number;
  stats: GroupStats;
}

export const athleteGroupStats = (filter: HistoryFilter, id: number) =>
  invoke<GroupStatsView>("athlete_group_stats", { filter, id });

/** Mete (`member = true`) o saca a un atleta de un grupo. */
export const setAthleteGroupMember = (id: number, runnerId: string, member: boolean) =>
  invoke<void>("set_athlete_group_member", { id, runnerId, member });

/** Guarda si las carreras propias cuentan en la vista de grupo. */
export const setIncludeSelf = (include: boolean) => invoke<void>("set_include_self", { include });

/** El atleta que se está viendo, sin volver a cargarlo; `null` = lo propio. */
export const viewedRunner = () => invoke<RunnerViewInfo | null>("viewed_runner");

export const getSettings = () => invoke<Settings>("get_settings");

export const saveSettings = (settings: Settings) => invoke<void>("save_settings", { settings });

/** Por qué no se pueden guardar unas zonas o, si se puede, los avisos sobre sus colores. */
export const checkZones = (zones: Zones) => invoke<string[]>("check_zones", { zones });

/** Paneles de análisis ocultos (#130), por identificador (`panels.tsx`). */
export const hiddenPanels = () => invoke<string[]>("hidden_panels");

export const setHiddenPanels = (ids: string[]) => invoke<void>("set_hidden_panels", { ids });

export const previewImport = (splPath: string, fitPath: string | null, identity: RunnerIdentity) =>
  invoke<ImportPreview>("preview_import", { splPath, fitPath, identity });

export const importRace = (request: ImportRequest) =>
  invoke<ImportOutcome>("import_race", { request });

export const importFolder = (folderPath: string) =>
  invoke<BatchSummary>("import_folder", { folderPath });

export const listRaces = () => invoke<RaceRow[]>("list_races");

export const raceDetail = (resultId: number) => invoke<RaceDetail>("race_detail", { resultId });

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

export const getHistory = (filter: HistoryFilter) => invoke<HistoryView>("history", { filter });

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
