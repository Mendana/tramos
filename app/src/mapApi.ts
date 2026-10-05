// Tipos y llamada del comando `race_map` (`src-tauri/src/race_map.rs`, `docs/app.md`, "Mapa").
// Todo viene calculado de Rust: la interfaz solo dibuja.
import { invoke } from "@tauri-apps/api/core";

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
  /** De 0 (más rápido) a 4 (más lento); `null` en un hueco del track. */
  pace_class: number | null;
  /** De 0 (más bajo) a 4 (más alto); `null` sin pulso o en un hueco. */
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

/** Límites de las clases, de menor a mayor (una más que clases). */
export interface ColorScale {
  edges: number[];
}

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
