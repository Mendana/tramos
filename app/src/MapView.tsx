// Mapa de la carrera (docs/app.md, "Mapa"): el track sobre OpenStreetMap, coloreado por ritmo o
// pulso, con las balizas y el tramo seleccionado sincronizado con la tabla de tramos.
//
// Todo lo calcula Rust (`race_map`); aquí solo se dibuja. MapLibre se carga aparte (este módulo
// se importa con `lazy`) para no engordar el arranque de la app.
import { ReactNode, useEffect, useRef, useState } from "react";
import { Map as MapLibreMap, Marker, setWorkerUrl } from "maplibre-gl";
import type { ExpressionSpecification, GeoJSONSourceSpecification } from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
// El worker de MapLibre se empaqueta como un fichero más de la app: así la CSP no tiene que
// aceptar workers de `blob:` (docs/datos-y-privacidad.md).
import workerUrl from "maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url";
import { LegReport, clock, codeLabel, signed, zoneLabel } from "./api";
import { ColorScale, MapControl, MapTrack, RaceMap, raceMap } from "./mapApi";
import { EmptyState, Notice, WatchIcon } from "./ui";

setWorkerUrl(workerUrl);

/** Teselas estándar de OpenStreetMap (https://operations.osmfoundation.org/policies/tiles/). */
const OSM_TILES = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
const OSM_ATTRIBUTION =
  '© <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener noreferrer">' +
  "OpenStreetMap contributors</a>";
/** Zoom máximo de las teselas de OSM. */
const MAX_ZOOM = 19;
/** Zoom máximo al encuadrar: a más, en un tramo corto apenas se ve el entorno. */
const FIT_MAX_ZOOM = 17;
const FIT_PADDING = 40;
/** Clases de la escala (`CLASSES` en `race_map.rs`). */
const CLASSES = 5;
/** Opacidad de los tramos no seleccionados cuando hay uno seleccionado. */
const DIMMED = 0.35;

type Metric = "pace" | "heart_rate";

const METRIC_PROPERTY: Record<Metric, string> = { pace: "pace", heart_rate: "hr" };

interface Props {
  resultId: number;
  /** Cambia cuando cambia el desfase del reloj: el mapa se vuelve a pedir y a pintar. */
  revision?: number;
  /** Tramos de la tabla, para describir el seleccionado. */
  legs: LegReport[];
  selected: number | null;
  onSelect: (leg: number) => void;
}

/** Tarjeta del mapa: carga `race_map` y enseña el mapa o por qué no lo hay. */
function MapView({ resultId, revision = 0, legs, selected, onSelect }: Props) {
  const [data, setData] = useState<RaceMap | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    setData(null);
    setError(null);
    raceMap(resultId)
      .then((d) => {
        if (current) setData(d);
      })
      .catch((err: unknown) => {
        if (current) setError(String(err));
      });
    return () => {
      current = false;
    };
  }, [resultId, revision]);

  if (error !== null) {
    return (
      <MapCard>
        <Notice kind="error">{error}</Notice>
      </MapCard>
    );
  }
  if (data === null) {
    return <MapPlaceholder />;
  }
  if (data.status === "no_track") {
    return (
      <MapCard>
        <EmptyState icon={<WatchIcon size={28} />} title="Sin track del reloj">
          <p className="muted">
            Esta carrera se importó sin el FIT del reloj, así que no hay recorrido que pintar.
            Vuelve a importarla con el FIT para verlo en el mapa.
          </p>
        </EmptyState>
      </MapCard>
    );
  }
  if (data.status === "not_aligned") {
    return (
      <MapCard>
        <Notice kind="error">
          {data.message} Corrige el desfase en «Reloj y cronometraje», encima del mapa.
        </Notice>
      </MapCard>
    );
  }
  if (data.bounds === null) {
    return (
      <MapCard>
        <EmptyState icon={<WatchIcon size={28} />} title="Ningún tramo en el track">
          <p className="muted">No se ha podido situar ningún tramo en el track del reloj.</p>
        </EmptyState>
      </MapCard>
    );
  }
  return <TrackMap map={data} legs={legs} selected={selected} onSelect={onSelect} />;
}

/** Hueco del mapa mientras se carga, con el mismo alto para que la página no salte. */
function MapPlaceholder() {
  return (
    <MapCard>
      <div className="map-frame map-loading">
        <span className="muted">Cargando el mapa…</span>
      </div>
    </MapCard>
  );
}

function MapCard({ aside, children }: { aside?: ReactNode; children: ReactNode }) {
  return (
    <div className="card card-flush map-card">
      <div className="card-title card-head">
        <h3>Mapa</h3>
        {aside}
      </div>
      {children}
    </div>
  );
}

function TrackMap({
  map: data,
  legs,
  selected,
  onSelect,
}: {
  map: MapTrack;
  legs: LegReport[];
  selected: number | null;
  onSelect: (leg: number) => void;
}) {
  const container = useRef<HTMLDivElement>(null);
  const mapRef = useRef<MapLibreMap | null>(null);
  const markers = useRef<{ control: MapControl; element: HTMLElement }[]>([]);
  const onSelectRef = useRef(onSelect);
  onSelectRef.current = onSelect;
  const [loaded, setLoaded] = useState(false);
  const [metric, setMetric] = useState<Metric>("pace");
  const scale = metric === "pace" ? data.pace : data.heart_rate;

  // El mapa se crea una vez por carrera.
  useEffect(() => {
    if (container.current === null || data.bounds === null) return;
    const map = new MapLibreMap({
      container: container.current,
      style: {
        version: 8,
        sources: {
          osm: {
            type: "raster",
            tiles: [OSM_TILES],
            tileSize: 256,
            maxzoom: MAX_ZOOM,
            attribution: OSM_ATTRIBUTION,
          },
        },
        layers: [{ id: "osm", type: "raster", source: "osm" }],
      },
      bounds: data.bounds,
      fitBoundsOptions: { padding: FIT_PADDING, maxZoom: FIT_MAX_ZOOM },
      maxZoom: MAX_ZOOM,
      attributionControl: { compact: false },
      // Política de teselas de OSM: nada de volver a pedir teselas caducadas mientras el mapa
      // está abierto; la caché del navegador sigue las cabeceras del servidor.
      refreshExpiredTiles: false,
      dragRotate: false,
      pitchWithRotate: false,
      touchPitch: false,
      locale: {
        "Map.Title": "Mapa de la carrera",
        "AttributionControl.ToggleAttribution": "Atribución",
      },
    });
    map.touchZoomRotate.disableRotation();
    map.keyboard.disableRotation();
    mapRef.current = map;

    map.on("load", () => {
      const colors = trackColors();
      map.addSource("legs", geojson(legFeatures(data)));
      map.addSource("pieces", geojson(pieceFeatures(data)));
      map.addLayer({
        id: "leg-halo",
        type: "line",
        source: "legs",
        filter: legIs(-1),
        layout: { "line-join": "round", "line-cap": "round" },
        paint: { "line-color": colors.selection, "line-width": 14, "line-opacity": 0.6 },
      });
      map.addLayer({
        id: "track-casing",
        type: "line",
        source: "pieces",
        layout: { "line-join": "round", "line-cap": "round" },
        paint: { "line-color": colors.casing, "line-width": 7 },
      });
      map.addLayer({
        id: "track-nodata",
        type: "line",
        source: "pieces",
        filter: ["<", ["get", METRIC_PROPERTY.pace], 0],
        layout: { "line-join": "round" },
        paint: { "line-color": colors.nodata, "line-width": 3, "line-dasharray": [2, 1.5] },
      });
      map.addLayer({
        id: "track",
        type: "line",
        source: "pieces",
        filter: [">=", ["get", METRIC_PROPERTY.pace], 0],
        layout: { "line-join": "round", "line-cap": "round" },
        paint: { "line-color": classColor("pace", data.pace, colors), "line-width": 4 },
      });
      // Zona de clic más ancha que la línea, un elemento por tramo.
      map.addLayer({
        id: "legs-hit",
        type: "line",
        source: "legs",
        layout: { "line-join": "round", "line-cap": "round" },
        paint: { "line-color": colors.selection, "line-width": 18, "line-opacity": 0 },
      });
      map.on("click", "legs-hit", (e) => {
        const leg = e.features?.[0]?.properties?.leg;
        if (typeof leg === "number") onSelectRef.current(leg);
      });
      map.on("mouseenter", "legs-hit", () => {
        map.getCanvas().classList.add("is-pointer");
      });
      map.on("mouseleave", "legs-hit", () => {
        map.getCanvas().classList.remove("is-pointer");
      });

      markers.current = data.controls.map((control) => {
        const element = controlElement(control);
        new Marker({ element }).setLngLat(control.coordinate).addTo(map);
        return { control, element };
      });
      setLoaded(true);
    });

    return () => {
      markers.current = [];
      mapRef.current = null;
      setLoaded(false);
      map.remove();
    };
  }, [data]);

  // Ritmo o pulso.
  useEffect(() => {
    const map = mapRef.current;
    if (!loaded || map === null) return;
    const property = METRIC_PROPERTY[metric];
    map.setFilter("track", [">=", ["get", property], 0]);
    map.setFilter("track-nodata", ["<", ["get", property], 0]);
    map.setPaintProperty("track", "line-color", classColor(metric, scale, trackColors()));
  }, [loaded, metric, scale]);

  // Tramo seleccionado: halo, el resto atenuado, sus balizas destacadas y encuadre si no se ve.
  useEffect(() => {
    const map = mapRef.current;
    if (!loaded || map === null) return;
    map.setFilter("leg-halo", legIs(selected ?? -1));
    const opacity: number | ExpressionSpecification =
      selected === null ? 1 : ["case", ["==", ["get", "leg"], selected], 1, DIMMED];
    for (const layer of ["track", "track-casing", "track-nodata"]) {
      map.setPaintProperty(layer, "line-opacity", opacity);
    }
    for (const { control, element } of markers.current) {
      const ends =
        selected !== null && (control.position === selected || control.position === selected - 1);
      element.classList.toggle("is-selected", ends);
      element.classList.toggle("is-dimmed", selected !== null && !ends);
    }
    const leg = data.legs.find((l) => l.index === selected);
    if (leg?.bounds) {
      const [west, south, east, north] = leg.bounds;
      const view = map.getBounds();
      const visible = view.contains([west, south]) && view.contains([east, north]);
      if (!visible) {
        map.fitBounds(leg.bounds, {
          padding: FIT_PADDING,
          maxZoom: FIT_MAX_ZOOM,
          duration: reducedMotion() ? 0 : 400,
        });
      }
    }
  }, [loaded, selected, data]);

  const zoom = (delta: number) => {
    const map = mapRef.current;
    map?.easeTo({ zoom: map.getZoom() + delta });
  };
  const fitAll = () => {
    if (data.bounds !== null) {
      mapRef.current?.fitBounds(data.bounds, { padding: FIT_PADDING, maxZoom: FIT_MAX_ZOOM });
    }
  };

  const selectedLeg = legs.find((l) => l.index === selected) ?? null;
  return (
    <MapCard
      aside={
        data.heart_rate !== null && (
          <div className="segmented" role="radiogroup" aria-label="Color del track">
            <label>
              <input
                type="radio"
                name="map-metric"
                checked={metric === "pace"}
                onChange={() => setMetric("pace")}
              />
              Ritmo
            </label>
            <label>
              <input
                type="radio"
                name="map-metric"
                checked={metric === "heart_rate"}
                onChange={() => setMetric("heart_rate")}
              />
              Pulso
            </label>
          </div>
        )
      }
    >
      <div className="map-frame">
        <div ref={container} className="map-canvas" />
        <div className="map-buttons">
          <button type="button" className="map-button" aria-label="Acercar" onClick={() => zoom(1)}>
            <PlusIcon />
          </button>
          <button type="button" className="map-button" aria-label="Alejar" onClick={() => zoom(-1)}>
            <MinusIcon />
          </button>
          <button
            type="button"
            className="map-button"
            aria-label="Ver toda la carrera"
            title="Ver toda la carrera"
            onClick={fitAll}
          >
            <FitIcon />
          </button>
        </div>
      </div>
      <div className="map-foot">
        <p className="small" aria-live="polite">
          {selectedLeg === null ? (
            <span className="muted">Elige un tramo en el mapa o en la tabla.</span>
          ) : (
            <LegSummary leg={selectedLeg} />
          )}
        </p>
        {scale !== null && (
          <Legend
            metric={metric}
            scale={scale}
            noData={data.pieces.some(
              (p) => (metric === "pace" ? p.pace_class : p.heart_rate_class) === null,
            )}
          />
        )}
      </div>
      {data.warnings.length > 0 && (
        <div className="map-notes">
          <Notice kind="warning">
            {data.warnings.map((w) => (
              <div key={w}>{w}</div>
            ))}
          </Notice>
        </div>
      )}
    </MapCard>
  );
}

function LegSummary({ leg }: { leg: LegReport }) {
  return (
    <>
      <strong>Tramo {leg.index}</strong>{" "}
      <span className="num">
        ({codeLabel(leg.from)} → {codeLabel(leg.to)}) · {clock(leg.split_s)}
      </span>
      {leg.loss_s !== null && (
        <span className={leg.is_error ? "loss-bad" : undefined}>
          {" "}
          · pérdida {signed(leg.loss_s)} s
        </span>
      )}
    </>
  );
}

/** Leyenda de la escala: las zonas del usuario o, sin ellas, cinco tonos de un mismo azul con
 *  los límites entre clases. */
function Legend({ metric, scale, noData }: { metric: Metric; scale: ColorScale; noData: boolean }) {
  const format = (v: number) => (metric === "pace" ? clock(v) : String(Math.round(v)));
  return (
    <figure className="map-legend">
      <figcaption className="small muted">
        {metric === "pace" ? "Ritmo (min/km)" : "Pulso (ppm)"}
        {scale.kind === "zones" && " · tus zonas"}
      </figcaption>
      {scale.kind === "zones" ? (
        <ul className="map-legend-zones small">
          {scale.colors.map((color, k) => (
            <li key={k}>
              {/* El color va en un atributo de SVG: la CSP no deja estilos en línea. */}
              <svg className="map-zone-swatch" viewBox="0 0 24 10" aria-hidden="true">
                <rect width="24" height="10" rx="2" fill={color} />
              </svg>
              <span className="num">{zoneLabel(scale.limits, k, format)}</span>
            </li>
          ))}
        </ul>
      ) : (
        <div className="map-legend-scale">
          <span className="small muted">{metric === "pace" ? "Más rápido" : "Más bajo"}</span>
          <div className="map-legend-bar">
            <div className="map-legend-ramp" aria-hidden="true">
              {Array.from({ length: CLASSES }, (_, k) => (
                <span key={k} className={`map-swatch map-swatch-${k}`} />
              ))}
            </div>
            <div className="map-legend-ticks small num">
              {scale.edges.slice(1, CLASSES).map((v, k) => (
                <span key={k}>{format(v)}</span>
              ))}
            </div>
          </div>
          <span className="small muted">{metric === "pace" ? "Más lento" : "Más alto"}</span>
        </div>
      )}
      {noData && (
        <div className="map-legend-nodata small muted">
          <span className="map-swatch-nodata" aria-hidden="true" /> Sin datos (hueco del track
          {metric === "heart_rate" && " o sin pulso"})
        </div>
      )}
    </figure>
  );
}

interface TrackColors {
  classes: string[];
  nodata: string;
  casing: string;
  selection: string;
}

/** Colores del mapa desde las variables de diseño (`--map-*` en `tokens.css`). */
function trackColors(): TrackColors {
  const css = getComputedStyle(document.documentElement);
  const read = (name: string) => css.getPropertyValue(name).trim();
  return {
    classes: Array.from({ length: CLASSES }, (_, k) => read(`--map-seq-${k + 1}`)),
    nodata: read("--map-nodata"),
    casing: read("--map-casing"),
    selection: read("--map-selection"),
  };
}

/** Color de cada trozo según su clase de `metric`: el de su zona o el tono de su clase. */
function classColor(
  metric: Metric,
  scale: ColorScale | null,
  colors: TrackColors,
): ExpressionSpecification {
  const palette = scale?.kind === "zones" ? scale.colors : colors.classes;
  const cases = palette.flatMap((color, k) => [k, color]);
  // La lista de pares clase → color es de longitud variable: TypeScript no la puede comprobar.
  return [
    "match",
    ["get", METRIC_PROPERTY[metric]],
    ...cases,
    colors.nodata,
  ] as unknown as ExpressionSpecification;
}

function legIs(leg: number): ExpressionSpecification {
  return ["==", ["get", "leg"], leg];
}

type FeatureCollection = Extract<GeoJSONSourceSpecification["data"], { type: "FeatureCollection" }>;

function geojson(data: FeatureCollection): GeoJSONSourceSpecification {
  return { type: "geojson", data };
}

function legFeatures(map: MapTrack): FeatureCollection {
  return {
    type: "FeatureCollection",
    features: map.legs
      .filter((l) => l.coordinates.length >= 2)
      .map((l) => ({
        type: "Feature",
        properties: { leg: l.index },
        geometry: { type: "LineString", coordinates: l.coordinates },
      })),
  };
}

/** Los trozos con su clase; -1 donde no hay (hueco o sin pulso). */
function pieceFeatures(map: MapTrack): FeatureCollection {
  return {
    type: "FeatureCollection",
    features: map.pieces.map((p) => ({
      type: "Feature",
      properties: {
        leg: p.leg,
        [METRIC_PROPERTY.pace]: p.pace_class ?? -1,
        [METRIC_PROPERTY.heart_rate]: p.heart_rate_class ?? -1,
      },
      geometry: { type: "LineString", coordinates: p.coordinates },
    })),
  };
}

const SVG_NS = "http://www.w3.org/2000/svg";

/** Baliza como en un mapa de orientación: triángulo (salida), círculo con su número o doble
 *  círculo (meta). Sin estilos en línea: todo va en clases de `map.css`. */
function controlElement(control: MapControl): HTMLElement {
  const element = document.createElement("div");
  element.className = `map-control map-control-${control.role}`;
  if (control.in_gap) element.classList.add("is-uncertain");
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 28 28");
  svg.setAttribute("class", "map-control-shape");
  svg.setAttribute("aria-hidden", "true");
  const shape = (tag: string, attributes: Record<string, string>) => {
    const node = document.createElementNS(SVG_NS, tag);
    for (const [key, value] of Object.entries(attributes)) node.setAttribute(key, value);
    svg.appendChild(node);
  };
  if (control.role === "start") {
    shape("polygon", { points: "14,3 25,22 3,22" });
  } else if (control.role === "finish") {
    shape("circle", { cx: "14", cy: "14", r: "11" });
    shape("circle", { cx: "14", cy: "14", r: "7" });
  } else {
    shape("circle", { cx: "14", cy: "14", r: "10" });
  }
  element.appendChild(svg);
  if (control.role === "control") {
    const label = document.createElement("span");
    label.className = "map-control-label";
    label.textContent = String(control.position);
    element.appendChild(label);
  }
  const name =
    control.role === "start"
      ? "Salida"
      : control.role === "finish"
        ? "Meta"
        : `Baliza ${control.position} (código ${control.code})`;
  element.setAttribute("aria-label", name);
  element.setAttribute("role", "img");
  return element;
}

function reducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** Icono de los botones del mapa, como los de `ui.tsx`. */
function ButtonIcon({ d }: { d: string }) {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={d} />
    </svg>
  );
}

const PlusIcon = () => <ButtonIcon d="M12 5v14M5 12h14" />;
const MinusIcon = () => <ButtonIcon d="M5 12h14" />;
const FitIcon = () => <ButtonIcon d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />;

export default MapView;
