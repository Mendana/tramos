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
  ideal_elapsed_s: number | null;
  behind_ideal_s: number | null;
  is_last: boolean;
  short_reference: boolean;
  excluded_from_patterns: boolean;
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

export const FORMAT_LABELS: Record<RaceFormat, string> = {
  sprint: "Sprint",
  middle: "Media",
  long: "Larga",
};

export function statusLabel(status: RaceStatus, place: number | null): string {
  if (status === "ok") return place === null ? "Clasificado" : `${place}.º`;
  if (status === "not_classified") return "No clasificado";
  if (status === "did_not_start") return "No presentado";
  return `Estado ${status.unknown}`;
}

export const coreVersion = () => invoke<string>("core_version");

export const storedIdentity = () => invoke<RunnerIdentity>("stored_identity");

export const previewImport = (
  splPath: string,
  fitPath: string | null,
  identity: RunnerIdentity,
) => invoke<ImportPreview>("preview_import", { splPath, fitPath, identity });

export const importRace = (request: ImportRequest) =>
  invoke<ImportOutcome>("import_race", { request });

export const listRaces = () => invoke<RaceRow[]>("list_races");

export const raceDetail = (resultId: number) =>
  invoke<RaceDetail>("race_detail", { resultId });

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
