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
