// Errores más comunes (P9, docs/historico.md): reparto de los errores por tipo y subtipo de la
// taxonomía, que se puede cruzar con el formato y con la duración del tramo (P7). Los repartos
// vienen todos calculados del núcleo; aquí solo se elige cuál dibujar.
import { useEffect, useState } from "react";
import {
  CommonErrors,
  ErrorTypes,
  FORMAT_LABELS,
  RaceFormat,
  Taxonomy,
  decimal,
  getTaxonomy,
} from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { ColumnChart } from "./charts/ColumnChart";
import { legsLabel, percent, tickPercent } from "./HistoryPanels";
import { bucketLabel } from "./LegLengthPanel";

const errorsLabel = (n: number) => `${n} ${n === 1 ? "error" : "errores"}`;
const seconds = (v: number) => `${decimal(v, 0)} s`;

/** Formato elegido: todos, uno o «sin formato». */
type FormatChoice = "all" | RaceFormat | "none";

function pick(data: CommonErrors, format: FormatChoice, bucket: number | null): ErrorTypes | null {
  const group =
    format === "all"
      ? { total: data.total, by_leg_length: data.by_leg_length }
      : data.by_format.find((f) => (format === "none" ? f.format === null : f.format === format));
  if (group === undefined) return null;
  if (bucket === null) return group.total;
  return group.by_leg_length[bucket]?.types ?? null;
}

/** Nombre de un tipo o subtipo; una clave que ya no está en la taxonomía, tal cual. */
function typeName(taxonomy: Taxonomy | null, key: string): string {
  return taxonomy?.types.find((t) => t.key === key)?.label ?? key;
}

function subtypeName(taxonomy: Taxonomy | null, type: string, key: string | null): string {
  if (key === null) return "Sin subtipo";
  const t = taxonomy?.types.find((x) => x.key === type);
  return t?.subtypes.find((s) => s.key === key)?.label ?? key;
}

/** Sección «Errores más comunes» de la vista histórica (P9). */
export function CommonErrorsPanel({ data }: { data: CommonErrors }) {
  const [taxonomy, setTaxonomy] = useState<Taxonomy | null>(null);
  const [format, setFormat] = useState<FormatChoice>("all");
  const [bucket, setBucket] = useState<number | null>(null);

  useEffect(() => {
    // Sin taxonomía se ven las claves: no hace falta avisar.
    getTaxonomy()
      .then(setTaxonomy)
      .catch(() => setTaxonomy(null));
  }, []);

  const shown = pick(data, format, bucket);
  // Solo los formatos con algún tramo, para no ofrecer opciones vacías.
  const formats = data.by_format.filter((f) => f.total.legs > 0);

  const controls = (
    <div className="row panel-controls">
      <label className="field">
        <span className="field-label">Formato</span>
        <select
          className="select"
          value={format}
          onChange={(e) => setFormat(e.target.value as FormatChoice)}
        >
          <option value="all">Todos</option>
          {formats.map((f) => (
            <option key={f.format ?? "none"} value={f.format ?? "none"}>
              {f.format === null ? "Sin formato" : FORMAT_LABELS[f.format]}
            </option>
          ))}
        </select>
      </label>
      <label className="field">
        <span className="field-label">Duración del tramo</span>
        <select
          className="select"
          value={bucket === null ? "" : String(bucket)}
          onChange={(e) => setBucket(e.target.value === "" ? null : Number(e.target.value))}
        >
          <option value="">Todas</option>
          {data.by_leg_length.map((b, i) => (
            <option key={b.from_s} value={String(i)}>
              {bucketLabel(b)}
            </option>
          ))}
        </select>
      </label>
    </div>
  );

  const t = shown ?? data.total;
  const share = (n: number) => (t.errors > 0 ? (n / t.errors) * 100 : null);
  const untyped =
    t.untyped > 0
      ? ` ${errorsLabel(t.untyped)} sin tipo${t.unreviewed > 0 ? ` (${t.unreviewed} sin revisar)` : ""}: etiquétalos en la tabla de tramos de cada carrera.`
      : "";
  const physical =
    t.physical_legs > 0
      ? ` Aparte, ${legsLabel(t.physical_legs)} ${t.physical_legs === 1 ? "marcado" : "marcados"} como físico (${seconds(t.physical_loss_s)} perdidos): no cuentan como error de orientación.`
      : "";
  const description =
    t.errors === 0
      ? `Ningún error en estos tramos.${physical}`
      : `Qué parte de tus ${errorsLabel(t.errors)} es de cada tipo.${untyped}${physical}`;

  const columns = [
    ...t.by_type.map((ty) => {
      const v = share(ty.errors);
      const name = typeName(taxonomy, ty.error_type);
      return {
        key: ty.error_type,
        label: name,
        sublabel: `n = ${ty.errors}`,
        value: v,
        tooltip: {
          value: v === null ? "—" : percent(v),
          detail: `${name} · ${errorsLabel(ty.errors)} · ${seconds(ty.loss_s)} perdidos`,
        },
      };
    }),
    ...(t.untyped > 0
      ? [
          {
            key: "untyped",
            label: "Sin tipo",
            sublabel: `n = ${t.untyped}`,
            value: share(t.untyped),
            color: "var(--chart-muted)",
            tooltip: {
              value: percent(share(t.untyped) ?? 0),
              detail: `Sin tipo · ${errorsLabel(t.untyped)} · ${seconds(t.untyped_loss_s)} perdidos`,
            },
          },
        ]
      : []),
  ];

  return (
    <>
      <h3 className="section-title">Errores más comunes</h3>
      <ChartPanel
        id="common-errors"
        title="Tipos de error"
        description={description}
        cases={`${errorsLabel(t.errors)} · ${legsLabel(t.legs)}`}
        chart={
          <>
            {controls}
            {t.errors === 0 ? (
              <p className="muted">Ningún error con estos filtros.</p>
            ) : (
              <ColumnChart
                label="Parte de los errores que es de cada tipo"
                formatTick={tickPercent}
                columns={columns}
              />
            )}
          </>
        }
        table={
          <>
            {controls}
            <table className="table">
              <thead>
                <tr>
                  <th>Tipo</th>
                  <th className="num">Errores</th>
                  <th className="num">% de los errores</th>
                  <th className="num">Pérdida</th>
                </tr>
              </thead>
              <tbody>
                {t.by_type.map((ty) => (
                  <TypeRows key={ty.error_type} ty={ty} taxonomy={taxonomy} share={share} />
                ))}
                {t.untyped > 0 && (
                  <tr>
                    <td>Sin tipo{t.unreviewed > 0 && ` (${t.unreviewed} sin revisar)`}</td>
                    <td className="num">{t.untyped}</td>
                    <td className="num">{percent(share(t.untyped) ?? 0)}</td>
                    <td className="num">{seconds(t.untyped_loss_s)}</td>
                  </tr>
                )}
                <tr className="total-row">
                  <td className="strong">Errores</td>
                  <td className="num">{t.errors}</td>
                  <td className="num">{t.errors > 0 ? "100 %" : "—"}</td>
                  <td className="num">{seconds(t.loss_s)}</td>
                </tr>
                {t.physical_legs > 0 && (
                  <tr>
                    <td className="muted">Físico (no es error)</td>
                    <td className="num muted">{t.physical_legs}</td>
                    <td className="num muted">—</td>
                    <td className="num muted">{seconds(t.physical_loss_s)}</td>
                  </tr>
                )}
              </tbody>
            </table>
          </>
        }
      />
    </>
  );
}

function TypeRows({
  ty,
  taxonomy,
  share,
}: {
  ty: ErrorTypes["by_type"][number];
  taxonomy: Taxonomy | null;
  share: (n: number) => number | null;
}) {
  const v = share(ty.errors);
  return (
    <>
      <tr>
        <td className="strong">{typeName(taxonomy, ty.error_type)}</td>
        <td className="num">{ty.errors}</td>
        <td className="num">{v === null ? "—" : percent(v)}</td>
        <td className="num">{seconds(ty.loss_s)}</td>
      </tr>
      {ty.subtypes.map((s) => {
        const sv = share(s.errors);
        return (
          <tr key={s.subtype ?? ""}>
            <td className="indent muted">{subtypeName(taxonomy, ty.error_type, s.subtype)}</td>
            <td className="num muted">{s.errors}</td>
            <td className="num muted">{sv === null ? "—" : percent(sv)}</td>
            <td className="num muted">{seconds(s.loss_s)}</td>
          </tr>
        );
      })}
    </>
  );
}
