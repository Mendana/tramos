// Tipos y llamada del comando `race_map` (`src-tauri/src/race_map.rs`, `docs/app.md`, "Mapa").
// Todo viene calculado de Rust: la interfaz solo dibuja.
import { invoke } from "@tauri-apps/api/core";
import type { Zones } from "./api";

/** `[longitud, latitud]`, como en GeoJSON. */
export type Coordinate = [number, number];

/** `[oeste, sur, este, norte]` en grados. */
export type Bounds = [number, number, number, number];

export type MissingLegTrack =
  | { reason: "no_punch_time"; code: number }
  | { reason: "outside_track"; code: number }
  | { reason: "out_of_order" };

export interface MapLeg {
  index: number;
  from: number;
  to: number;
  coordinates: Coordinate[];
  bounds: Bounds | null;
  missing: MissingLegTrack | null;
}

/** Trozo del track con la misma clase de ritmo y de pulso, dentro de un tramo. */
export interface TrackPiece {
  leg: number;
  coordinates: Coordinate[];
  /** Clase o zona, de 0 (más rápido) a la última (más lento); `null` en un hueco del track. */
  pace_class: number | null;
  /** Clase o zona, de 0 (más bajo) a la última (más alto); `null` sin pulso o en un hueco. */
  heart_rate_class: number | null;
}

export interface MapControl {
  /** 0 = salida, 1 = primera baliza, …, la última = meta. El tramo `position` acaba aquí. */
  position: number;
  code: number;
  role: "start" | "control" | "finish";
  coordinate: Coordinate;
  in_gap: boolean;
}

/** Escala del track: clases por cuantiles de la carrera o las zonas del usuario (#96). */
export type ColorScale =
  /** Cinco clases; `edges`: sus límites, de menor a mayor (uno más que clases). */
  { kind: "quantiles"; edges: number[] } | ({ kind: "zones" } & Zones);

export interface MapTrack {
  bounds: Bounds | null;
  legs: MapLeg[];
  pieces: TrackPiece[];
  controls: MapControl[];
  /** s/km. */
  pace: ColorScale | null;
  /** ppm. */
  heart_rate: ColorScale | null;
  warnings: string[];
}

export type RaceMap =
  | { status: "no_track" }
  | { status: "not_aligned"; message: string }
  | ({ status: "ready" } & MapTrack);

export const raceMap = (resultId: number) => invoke<RaceMap>("race_map", { resultId });
