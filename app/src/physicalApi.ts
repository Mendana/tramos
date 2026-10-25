// Tipos y llamada del comando `race_physical` (`src-tauri/src/race_physical.rs`,
// `docs/rendimiento-fisico.md`). Todo viene calculado de Rust: la interfaz solo dibuja.
import { invoke } from "@tauri-apps/api/core";
import type { Zones } from "./api";

/** Velocidad con la que se compara el traslado: en llano, o la real si falta la altitud. */
export type SpeedKind = "flat" | "moving";

export type GradeClass = "steep_down" | "down" | "flat" | "up" | "steep_up";

/** Cifras de un conjunto de trozos de traslado. */
export interface TransitStats {
  transit_s: number;
  distance_m: number;
  speed_mps: number | null;
  flat_speed_mps: number | null;
  heart_rate_bpm: number | null;
  efficiency_m_per_beat: number | null;
  cadence_spm: number | null;
  stride_m: number | null;
}

/** Una décima del traslado. */
export interface DriftBlock extends TransitStats {
  index: number;
  from_s: number;
  to_s: number;
}

export interface GradeStats extends TransitStats {
  class: GradeClass;
}

export interface HeartRateZoneTimes {
  /** Segundos en cada zona (una más que límites). */
  seconds: number[];
  /** Minutos en cada zona por su número (1, 2, 3…), sumados. */
  load: number;
}

export interface RacePhysical {
  duration_s: number;
  distance_m: number;
  ascent_m: number | null;
  descent_m: number | null;
  moving_s: number;
  stopped_s: number;
  gap_s: number;
  attack_s: number;
  speed: SpeedKind;
  transit: TransitStats;
  heart_rate_bpm: number | null;
  heart_rate_max_bpm: number | null;
  halves: [TransitStats, TransitStats] | null;
  decoupling_pct: number | null;
  speed_change_pct: number | null;
  heart_rate_change_bpm: number | null;
  stride_change_pct: number | null;
  blocks: DriftBlock[];
  by_grade: GradeStats[];
  vam_m_per_h: number | null;
  climb_s: number;
  heart_rate_zones: HeartRateZoneTimes | null;
}

export type SeriesState = "transit" | "attack" | "stopped" | "gap";

/** Un instante de la serie; `null` donde no se puede dar. */
export interface SeriesSample {
  t_s: number;
  distance_m: number;
  altitude_m: number | null;
  speed_mps: number | null;
  flat_speed_mps: number | null;
  heart_rate_bpm: number | null;
  cadence_spm: number | null;
  stride_m: number | null;
  leg: number | null;
  state: SeriesState;
}

export interface SeriesLeg {
  index: number;
  from: number;
  to: number;
  from_s: number;
  to_s: number;
  from_m: number;
  to_m: number;
}

export interface SeriesControl {
  position: number;
  code: number;
  t_s: number;
  distance_m: number;
}

export interface RaceSeries {
  step_s: number;
  duration_s: number;
  samples: SeriesSample[];
  legs: SeriesLeg[];
  controls: SeriesControl[];
}

export interface PhysicalReady {
  physical: RacePhysical;
  series: RaceSeries;
  /** Las zonas de pulso de Ajustes, con sus colores. */
  zones: Zones | null;
}

export type PhysicalView =
  | { status: "no_track" }
  | { status: "not_aligned"; message: string }
  | ({ status: "ready" } & PhysicalReady);

export const racePhysical = (resultId: number) =>
  invoke<PhysicalView>("race_physical", { resultId });
