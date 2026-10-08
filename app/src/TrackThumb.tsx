// Miniatura del track de una carrera (Inicio): el recorrido tal y como lo devuelve `race_map`,
// dibujado en SVG sin teselas ni MapLibre. Solo dibuja: las coordenadas vienen calculadas de Rust.
import { useEffect, useState } from "react";
import { Coordinate, MapTrack, raceMap } from "./mapApi";

/** Margen alrededor del track, en fracción del lado mayor. */
const PADDING = 0.06;

/** Proyección equirrectangular con el coseno de la latitud media: basta para unos km. */
function projection(track: MapTrack) {
  const [west, south, east, north] = track.bounds ?? [0, 0, 1, 1];
  const kx = Math.cos((((south + north) / 2) * Math.PI) / 180);
  const width = Math.max((east - west) * kx, 1e-6);
  const height = Math.max(north - south, 1e-6);
  const pad = Math.max(width, height) * PADDING;
  const point = ([lon, lat]: Coordinate) => `${(lon - west) * kx},${north - lat}`;
  return {
    point,
    viewBox: `${-pad} ${-pad} ${width + 2 * pad} ${height + 2 * pad}`,
    size: Math.max(width, height),
  };
}

/** El track de una carrera, o `null` si no tiene (o aún no ha llegado, o no se puede situar). */
export function useMapTrack(resultId: number, revision = 0): MapTrack | null {
  const [track, setTrack] = useState<MapTrack | null>(null);
  useEffect(() => {
    setTrack(null);
    let current = true;
    raceMap(resultId)
      .then((map) => {
        if (current && map.status === "ready" && map.bounds !== null) setTrack(map);
      })
      // Sin miniatura no pasa nada: la carrera se abre igual.
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [resultId, revision]);
  return track;
}

/** Miniatura del track de una carrera; nada si no tiene. */
export function TrackThumb({ resultId }: { resultId: number }) {
  const track = useMapTrack(resultId);
  return track === null ? null : <TrackSvg track={track} />;
}

/** El recorrido, el track y las balizas, en SVG. */
export function TrackSvg({ track }: { track: MapTrack }) {
  const { point, viewBox, size } = projection(track);
  const radius = size * 0.022;
  return (
    <svg
      className="track-thumb"
      viewBox={viewBox}
      preserveAspectRatio="xMidYMid meet"
      role="img"
      aria-label="Recorrido de la carrera según el reloj"
    >
      {/* El recorrido: de baliza en baliza, en el magenta de los mapas de orientación. */}
      <polyline
        className="track-thumb-course"
        points={track.controls.map((c) => point(c.coordinate)).join(" ")}
        vectorEffect="non-scaling-stroke"
      />
      {track.pieces.map((piece, i) => (
        <polyline
          key={`casing-${i}`}
          className="track-thumb-casing"
          points={piece.coordinates.map(point).join(" ")}
          vectorEffect="non-scaling-stroke"
        />
      ))}
      {track.pieces.map((piece, i) => (
        <polyline
          key={`piece-${i}`}
          className="track-thumb-line"
          points={piece.coordinates.map(point).join(" ")}
          vectorEffect="non-scaling-stroke"
        />
      ))}
      {track.controls.map((control) => {
        const [x, y] = point(control.coordinate).split(",").map(Number);
        return (
          <circle
            key={control.position}
            className="track-thumb-control"
            cx={x}
            cy={y}
            r={control.role === "finish" ? radius * 1.3 : radius}
            vectorEffect="non-scaling-stroke"
          />
        );
      })}
    </svg>
  );
}
