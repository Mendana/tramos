// Una carrera con todos los atletas que la corrieron (#163, docs/app.md, "Atletas"): la tabla de
// atletas y, por recorrido, la diferencia con el tiempo ideal y la pérdida por tramo (como
// «Frente al grupo», P4), los tramos que fallaron varios, la elección de ruta (#190) y el mapa
// con sus tracks. Cada atleta se mide contra su propio ritmo; todo viene calculado de Rust
// (`athlete_race`).
import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import {
  AthleteRaceView,
  FORMAT_LABELS,
  RaceAthleteRow,
  RaceCourseView,
  RouteChoice,
  SHARE_LABELS,
  Taxonomy,
  athleteRace,
  clock,
  codeLabel,
  decimal,
  getTaxonomy,
  signed,
  statusLabel,
} from "./api";
import { athleteName } from "./AthleteRacesScreen";
import { ChartPanel, PanelsOpen } from "./charts/ChartPanel";
import { GroupedColumnChart } from "./charts/GroupedColumnChart";
import { MultiLineChart } from "./charts/MultiLineChart";
import { SERIES_COLORS } from "./charts/common";
import { typeLabel } from "./LegTags";
import { ChevronRight, Notice, PageHeader, TitleWithHelp } from "./ui";

// MapLibre pesa: se carga solo si hay tracks.
const CourseTrackMap = lazy(() => import("./MapView").then((m) => ({ default: m.CourseTrackMap })));

/** Atletas a la vez en las gráficas y el mapa: uno por color de la paleta. */
const MAX_SHOWN = SERIES_COLORS.length;

/** Diferencia con el ideal: `+1:23` por detrás, `-0:30` por delante. */
const behind = (v: number | null) => (v === null ? "—" : `${v > 0 ? "+" : ""}${clock(v)}`);
const loss = (v: number | null) => (v === null ? "—" : `${signed(v)} s`);

/** «Opción A», «Opción B»…, de la más rápida a la más lenta. */
const optionName = (i: number) => `Opción ${String.fromCharCode(65 + i)}`;

/** Lo que compartió, en corto. */
const levelLabel = (a: RaceAthleteRow) => (a.level === null ? "Tuya" : SHARE_LABELS[a.level]);

/** Por qué un atleta no sale en todas las gráficas, si es el caso. */
function athleteNote(a: RaceAthleteRow): string | null {
  if (a.problem !== null) return a.problem;
  if (a.course === null) return "Solo compartió el resumen: no sale en las gráficas ni en el mapa.";
  if (!a.on_map) return a.level === "legs" ? "Sin track: no sale en el mapa." : null;
  return null;
}

function AthleteRaceScreen({
  raceId,
  onOpenRace,
}: {
  raceId: string;
  /** Abre la carrera de un atleta (o la propia) en su vista de carrera. */
  onOpenRace: (athlete: RaceAthleteRow) => void;
}) {
  const [view, setView] = useState<AthleteRaceView | null>(null);
  const [taxonomy, setTaxonomy] = useState<Taxonomy | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Recorrido que se ve (posición en `courses`).
  const [course, setCourse] = useState(0);

  useEffect(() => {
    setView(null);
    setError(null);
    setCourse(0);
    athleteRace(raceId)
      .then(setView)
      .catch((err: unknown) => setError(String(err)));
    getTaxonomy()
      .then(setTaxonomy)
      .catch(() => setTaxonomy(null));
  }, [raceId]);

  if (error !== null) return <Notice kind="error">{error}</Notice>;
  if (view === null) return <p className="muted">Cargando…</p>;

  const count = view.athletes.length;
  const current = view.courses[course] ?? null;
  return (
    <>
      <PageHeader
        title={view.name ?? "Carrera sin nombre"}
        subtitle={
          <>
            <span className="num">{view.date}</span>
            {view.format !== null && (
              <span className="pill pill-accent">{FORMAT_LABELS[view.format]}</span>
            )}
            <span>
              {count} {count === 1 ? "atleta" : "atletas"}
            </span>
          </>
        }
      />

      <AthletesTable athletes={view.athletes} onOpenRace={onOpenRace} />

      {count === 1 && (
        <Notice>
          Solo {athleteName(view.athletes[0])} ha compartido esta carrera: no hay con quién
          compararle. Cuando otro atleta la comparta, saldrá aquí.
        </Notice>
      )}

      {view.courses.length > 1 && (
        <div className="card row">
          <label className="field">
            <span className="field-label">Recorrido</span>
            <select
              className="select"
              value={course}
              onChange={(e) => setCourse(Number(e.target.value))}
            >
              {view.courses.map((c, i) => (
                <option key={i} value={i}>
                  {c.class_names.join(", ")} · {c.runners.length}{" "}
                  {c.runners.length === 1 ? "atleta" : "atletas"}
                </option>
              ))}
            </select>
          </label>
          <p className="field-hint">
            Corrieron recorridos distintos: las gráficas por tramo y el mapa son de un recorrido.
          </p>
        </div>
      )}

      {current !== null && (
        <CourseSection
          // Otro recorrido, otros atletas y otros tramos: se empieza de cero.
          key={course}
          course={current}
          athletes={view.athletes}
          taxonomy={taxonomy}
        />
      )}
    </>
  );
}

/** Atleta, categoría, puesto, tiempo, tiempo perdido, errores y lo que compartió. */
function AthletesTable({
  athletes,
  onOpenRace,
}: {
  athletes: RaceAthleteRow[];
  onOpenRace: (athlete: RaceAthleteRow) => void;
}) {
  return (
    <section>
      <h3 className="section-title">Atletas</h3>
      <div className="card card-flush">
        <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th>Atleta</th>
                <th>Categoría</th>
                <th>Resultado</th>
                <th className="num">Tiempo</th>
                <th className="num">Perdido</th>
                <th>Compartió</th>
                <th>
                  <span className="visually-hidden">Abrir</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {athletes.map((a) => {
                const s = a.summary;
                const note = athleteNote(a);
                const open = a.result_id !== null ? () => onOpenRace(a) : undefined;
                return (
                  <tr
                    key={`${a.is_self ? "self" : a.runner.runner_id}`}
                    className={open === undefined ? undefined : "clickable"}
                    tabIndex={open === undefined ? undefined : 0}
                    title={open === undefined ? undefined : `Abrir la carrera de ${athleteName(a)}`}
                    onClick={open}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") open?.();
                    }}
                  >
                    <td>
                      <div className="strong">{athleteName(a)}</div>
                      {note !== null && <div className="small muted">{note}</div>}
                    </td>
                    <td>{s?.class_name ?? "—"}</td>
                    <td>
                      {s === null ? "—" : statusLabel(s.status, s.place)}
                      {a.course_place !== null && (
                        <div className="small muted">{a.course_place}.º del recorrido</div>
                      )}
                    </td>
                    <td className="num">{clock(s?.total_s ?? null)}</td>
                    <td className="num">
                      <span className={s !== null && s.error_count > 0 ? "loss-bad" : undefined}>
                        {clock(s?.lost_time_s ?? null)}
                      </span>
                      {s !== null && s.error_count > 0 && (
                        <div className="small muted">
                          {s.error_count} {s.error_count === 1 ? "error" : "errores"}
                        </div>
                      )}
                    </td>
                    <td>
                      <span className="pill">{levelLabel(a)}</span>
                    </td>
                    <td className="chevron">{open !== undefined && <ChevronRight />}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}

/** Ficha de un atleta: su color de serie (en un atributo de SVG, por la CSP) y su nombre. */
function Swatch({ color }: { color: string }) {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
      <rect width="10" height="10" rx="2" fill={color} />
    </svg>
  );
}

/** Un recorrido: atletas elegidos, las dos gráficas de P4, los tramos que fallaron varios y el
 *  mapa. */
function CourseSection({
  course,
  athletes,
  taxonomy,
}: {
  course: RaceCourseView;
  athletes: RaceAthleteRow[];
  taxonomy: Taxonomy | null;
}) {
  // Los que se ven (posiciones en `athletes`), uno por hueco de la paleta: el hueco da el color,
  // así que quitar a uno no repinta a los demás (#178). De entrada, los primeros del recorrido.
  const [chosen, setChosen] = useState<(number | null)[]>(() =>
    Array.from({ length: MAX_SHOWN }, (_, i) => course.runners[i]?.runner ?? null),
  );
  const [selectedLeg, setSelectedLeg] = useState<number | null>(null);
  // Opción de ruta resaltada en el mapa (posición en las opciones del tramo elegido).
  const [routeOption, setRouteOption] = useState<number | null>(null);
  const selectLeg = (leg: number | null) => {
    setSelectedLeg(leg);
    setRouteOption(null);
  };
  const toggleLeg = (leg: number) => selectLeg(selectedLeg === leg ? null : leg);
  const choice = course.routes.find((r) => r.leg === selectedLeg) ?? null;
  // Memorizado: el mapa vuelve a pintar el tramo cada vez que cambia.
  const highlighted = useMemo(
    () =>
      choice === null || routeOption === null
        ? null
        : (choice.options[routeOption]?.runners.map((r) => r.runner) ?? null),
    [choice, routeOption],
  );
  const shown = course.runners
    .filter((r) => chosen.includes(r.runner))
    .map((r) => ({
      runner: r,
      athlete: athletes[r.runner],
      label: athleteName(athletes[r.runner]),
      color: SERIES_COLORS[chosen.indexOf(r.runner)],
    }));
  const colorOf = (athlete: number) => shown.find((s) => s.runner.runner === athlete)?.color;
  const many = course.runners.length > MAX_SHOWN;
  const toggle = (athlete: number) =>
    setChosen((c) => {
      const slot = c.includes(athlete) ? c.indexOf(athlete) : c.indexOf(null);
      if (slot === -1) return c;
      return c.map((a, i) => (i === slot ? (a === athlete ? null : athlete) : a));
    });
  const legName = (i: number) => {
    const leg = course.legs[i];
    return `Tramo ${leg.index} · ${codeLabel(leg.from)} → ${codeLabel(leg.to)}`;
  };
  const cases = `${shown.length} ${shown.length === 1 ? "atleta" : "atletas"}`;

  // Las fichas hacen de leyenda; con más atletas que colores, también eligen.
  const picker = (
    <div
      className="compare-picker"
      role={many ? "group" : undefined}
      aria-label={many ? "Atletas que salen" : undefined}
    >
      {course.runners.map((r) => {
        const athlete = athletes[r.runner];
        const color = colorOf(r.runner);
        return many ? (
          <button
            key={r.runner}
            type="button"
            className="chip"
            aria-pressed={color !== undefined}
            disabled={color === undefined && !chosen.includes(null)}
            onClick={() => toggle(r.runner)}
          >
            {color !== undefined && <Swatch color={color} />}
            {athleteName(athlete)}
          </button>
        ) : (
          <span className="compare-chip" key={r.runner}>
            {color !== undefined && <Swatch color={color} />}
            {athleteName(athlete)}
          </span>
        );
      })}
    </div>
  );

  const table = (values: (r: (typeof shown)[number], i: number) => string) => (
    <table className="table">
      <thead>
        <tr>
          <th className="num">Tramo</th>
          <th>Balizas</th>
          {shown.map((s) => (
            <th className="num" key={s.runner.runner}>
              {s.label}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {course.legs.map((leg, i) => (
          <tr key={leg.index}>
            <td className="num">{leg.index}</td>
            <td className="num">
              {codeLabel(leg.from)} → {codeLabel(leg.to)}
            </td>
            {shown.map((s) => (
              <td className="num" key={s.runner.runner}>
                {values(s, i)}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );

  const showMap = (leg: number) => {
    selectLeg(leg);
    document.getElementById("athletes-race-map")?.scrollIntoView({
      block: "start",
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
    });
  };
  const onMap = course.runners.filter((r) => athletes[r.runner].on_map).length;

  const notOnMap = course.runners
    .map((r) => athletes[r.runner])
    .filter((a) => !a.on_map)
    .map(athleteName);

  return (
    <section className="stack">
      <h3 className="section-title">
        {course.class_names.length > 0
          ? `Recorrido de ${course.class_names.join(", ")}`
          : "Recorrido"}
      </h3>
      {course.weak_reference && (
        <Notice kind="warning">
          Pocos corredores en este recorrido: la referencia es débil y los números, frágiles.
        </Notice>
      )}
      {many && (
        <p className="small muted">
          Hay {course.runners.length} atletas en este recorrido: elige hasta {MAX_SHOWN} para las
          gráficas y el mapa.
        </p>
      )}
      <PanelsOpen.Provider value={true}>
        <ChartPanel
          id="athletes-race-ideal"
          help={{ page: "carreras-atletas", section: "Diferencia con el tiempo ideal" }}
          title="Diferencia con el tiempo ideal"
          description="Tiempo por detrás del ideal del recorrido al acabar cada tramo: cuanto más abajo, más lejos del ideal; una línea que baja de golpe es un tramo malo."
          cases={cases}
          chart={
            <>
              {picker}
              <MultiLineChart
                label="Diferencia acumulada respecto al tiempo ideal, por tramo, de cada atleta"
                xLabels={["S", ...course.legs.map((l) => String(l.index))]}
                tooltipTitle={(i) =>
                  i === 0
                    ? { value: "Salida", detail: "Todos empiezan en 0" }
                    : { value: legName(i - 1), detail: "Por detrás del ideal" }
                }
                // Hacia abajo es por detrás, como en la gráfica clásica de WinSplits.
                formatTick={(v) => `${Math.round(-v) === 0 ? 0 : Math.round(-v)} s`}
                formatValue={(v) => behind(v === null ? null : -v)}
                series={shown.map((s) => ({
                  key: String(s.runner.runner),
                  label: s.label,
                  color: s.color,
                  emphasis: s.athlete.is_self,
                  values: [0, ...s.runner.behind_ideal_s.map((v) => (v === null ? null : -v))],
                }))}
              />
            </>
          }
          table={table((s, i) => behind(s.runner.behind_ideal_s[i]))}
        />
        <ChartPanel
          id="athletes-race-loss"
          help={{ page: "carreras-atletas", section: "Pérdida por tramo comparada" }}
          title="Pérdida por tramo comparada"
          description="Segundos perdidos (arriba) o ganados (abajo) en cada tramo frente a lo esperado con el rendimiento habitual de cada uno: cada atleta, contra su propio ritmo."
          cases={cases}
          chart={
            <>
              {picker}
              <GroupedColumnChart
                label="Pérdida en segundos de cada tramo, por atleta"
                xLabels={course.legs.map((l) => String(l.index))}
                tooltipTitle={(i) => ({
                  value: legName(i),
                  detail: "Pérdida frente a lo esperado",
                })}
                formatTick={(v) => `${Math.round(v)} s`}
                formatValue={loss}
                series={shown.map((s) => ({
                  key: String(s.runner.runner),
                  label: s.label,
                  color: s.color,
                  values: s.runner.loss_s,
                }))}
              />
            </>
          }
          table={table(
            (s, i) => `${loss(s.runner.loss_s[i])}${s.runner.is_error[i] ? " · error" : ""}`,
          )}
        />
      </PanelsOpen.Provider>

      <SharedErrors
        course={course}
        athletes={athletes}
        taxonomy={taxonomy}
        onMap={course.map === null ? null : showMap}
      />

      {onMap >= 2 && (
        <RouteChoices
          routes={course.routes}
          athletes={athletes}
          selected={choice}
          option={routeOption}
          onLeg={(leg) => (leg === selectedLeg ? selectLeg(null) : showMap(leg))}
          onOption={(i) => setRouteOption((o) => (o === i ? null : i))}
        />
      )}

      <div id="athletes-race-map">
        {course.map === null ? (
          <div className="card">
            <h3>Mapa</h3>
            <p className="muted">
              Ningún atleta de este recorrido ha compartido su track del reloj: no hay mapa.
            </p>
          </div>
        ) : (
          <>
            <Suspense
              fallback={
                <div className="card card-flush map-card">
                  <div className="map-frame map-loading">
                    <span className="muted">Cargando el mapa…</span>
                  </div>
                </div>
              }
            >
              <CourseTrackMap
                map={course.map}
                series={shown
                  .filter((s) => s.athlete.on_map)
                  .map((s) => ({ athlete: s.runner.runner, label: s.label, color: s.color }))}
                selected={selectedLeg}
                onSelect={toggleLeg}
                highlighted={highlighted}
                highlightLabel={routeOption === null ? null : optionName(routeOption)}
              />
            </Suspense>
            {notOnMap.length > 0 && (
              <p className="small muted">No salen en el mapa: {notOnMap.join(", ")}.</p>
            )}
          </>
        )}
      </div>
    </section>
  );
}

/** Los tramos en los que los atletas con track fueron por sitios distintos (#190) y, con uno
 *  elegido, sus opciones de ruta. */
function RouteChoices({
  routes,
  athletes,
  selected,
  option,
  onLeg,
  onOption,
}: {
  routes: RouteChoice[];
  athletes: RaceAthleteRow[];
  /** La elección del tramo elegido en el mapa, si la tiene. */
  selected: RouteChoice | null;
  option: number | null;
  onLeg: (leg: number) => void;
  onOption: (option: number) => void;
}) {
  return (
    <div className="card">
      <TitleWithHelp
        title="Elección de ruta"
        text="Los tramos en los que los atletas con track fueron por sitios distintos. Dos rutas que van a menos de 30 m de media son la misma opción; las opciones van de la más rápida a la más lenta, por la mediana de sus tiempos."
        help={{ page: "carreras-atletas", section: "Elección de ruta" }}
      />
      {routes.length === 0 ? (
        <p className="muted">En ningún tramo cogieron rutas distintas.</p>
      ) : (
        <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th className="num">Tramo</th>
                <th>Balizas</th>
                <th className="num">Opciones</th>
                <th className="num">Mejor frente a peor</th>
              </tr>
            </thead>
            <tbody>
              {routes.map((r) => (
                <tr
                  key={r.leg}
                  className={r.leg === selected?.leg ? "clickable is-selected" : "clickable"}
                  tabIndex={0}
                  title="Ver sus opciones en el mapa"
                  onClick={() => onLeg(r.leg)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") onLeg(r.leg);
                  }}
                >
                  <td className="num">{r.leg}</td>
                  <td className="num">
                    {codeLabel(r.from)} → {codeLabel(r.to)}
                  </td>
                  <td className="num">{r.options.length}</td>
                  <td className="num">{r.spread_s === null ? "—" : `+${clock(r.spread_s)}`}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {selected !== null && (
        <>
          <h4>
            Tramo {selected.leg} · {codeLabel(selected.from)} → {codeLabel(selected.to)}
          </h4>
          <p className="small muted">
            Elige una opción para resaltar sus rutas en el mapa. «Con error» es un tramo que ese
            atleta falló: su tiempo dice más del error que de la ruta.
          </p>
          <div className="table-wrap">
            <table className="table">
              <thead>
                <tr>
                  <th>Opción</th>
                  <th className="num">Mediana</th>
                  <th className="num">Distancia</th>
                  <th className="num">Subida</th>
                  <th>Atletas</th>
                </tr>
              </thead>
              <tbody>
                {selected.options.map((o, i) => (
                  <tr key={i}>
                    <td>
                      <button
                        type="button"
                        className="chip"
                        aria-pressed={option === i}
                        onClick={() => onOption(i)}
                      >
                        {optionName(i)}
                      </button>
                    </td>
                    <td className="num">{clock(o.median_split_s)}</td>
                    <td className="num">
                      {o.distance_m === null ? "—" : `${decimal(o.distance_m / 1000, 2)} km`}
                    </td>
                    <td className="num">
                      {o.ascent_m === null ? "—" : `${Math.round(o.ascent_m)} m`}
                    </td>
                    <td>
                      {o.runners.map((r, k) => (
                        <span key={r.runner}>
                          {k > 0 && ", "}
                          {athleteName(athletes[r.runner])}{" "}
                          <span className="num">{clock(r.split_s)}</span>
                          {r.is_error && <span className="loss-bad"> (con error)</span>}
                        </span>
                      ))}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </div>
  );
}

/** Los tramos con error en dos o más atletas, con su pérdida y su etiqueta. */
function SharedErrors({
  course,
  athletes,
  taxonomy,
  onMap,
}: {
  course: RaceCourseView;
  athletes: RaceAthleteRow[];
  taxonomy: Taxonomy | null;
  onMap: ((leg: number) => void) | null;
}) {
  if (course.runners.length < 2) return null;
  return (
    <div className="card">
      <TitleWithHelp
        title="Tramos que fallaron varios"
        text="Los tramos con error en dos atletas o más. Qué es error lo dice la etiqueta de cada uno y, si no la tiene, el cálculo."
        help={{ page: "carreras-atletas", section: "Tramos que fallaron varios" }}
      />
      {course.shared_errors.length === 0 ? (
        <p className="muted">Ningún tramo lo fallaron dos o más.</p>
      ) : (
        <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th className="num">Tramo</th>
                <th>Balizas</th>
                <th>Quién y qué</th>
                {onMap !== null && (
                  <th>
                    <span className="visually-hidden">Mapa</span>
                  </th>
                )}
              </tr>
            </thead>
            <tbody>
              {course.shared_errors.map((leg) => (
                <tr key={leg.leg}>
                  <td className="num">{leg.leg}</td>
                  <td className="num">
                    {codeLabel(leg.from)} → {codeLabel(leg.to)}
                  </td>
                  <td>
                    {leg.errors.map((e) => {
                      const kind = e.tag === null ? null : typeLabel(taxonomy, e.tag);
                      return (
                        <div key={e.runner}>
                          <span className="strong">{athleteName(athletes[e.runner])}</span>{" "}
                          <span className="loss-bad num">{loss(e.loss_s)}</span>
                          {kind !== null && <span> · {kind}</span>}
                          {!e.reviewed && <span className="muted"> · sin revisar</span>}
                          {e.tag?.note != null && e.tag.note !== "" && (
                            <span className="muted"> · «{e.tag.note}»</span>
                          )}
                        </div>
                      );
                    })}
                  </td>
                  {onMap !== null && (
                    <td>
                      <button
                        type="button"
                        className="btn btn-ghost"
                        onClick={() => onMap(leg.leg)}
                      >
                        Ver en el mapa
                      </button>
                    </td>
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

export default AthleteRaceScreen;
