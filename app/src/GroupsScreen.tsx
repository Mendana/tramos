// Grupos de atletas (#120, docs/app.md, "Atletas"): quien entrena los crea, les pone nombre,
// descripción y color, y mete o saca atletas. Cada grupo lleva a sus estadísticas (#145), con
// todos sus miembros juntos. Se guardan en la base propia; los miembros van por el identificador
// de sus paquetes.
import { useCallback, useEffect, useState } from "react";
import {
  GROUP_COLORS,
  GroupCandidate,
  GroupFields,
  GroupInfo,
  GroupsView,
  athleteGroups,
  createAthleteGroup,
  deleteAthleteGroup,
  setAthleteGroupMember,
  updateAthleteGroup,
} from "./api";
import { ChartIcon, EmptyState, GroupIcon, Notice, PageHeader, PencilIcon } from "./ui";

/** Punto con el color de un grupo. El color va en un atributo SVG: la CSP no deja estilos. */
export function GroupDot({ color, size = 12 }: { color: string; size?: number }) {
  return (
    <svg className="group-dot" width={size} height={size} viewBox="0 0 12 12" aria-hidden="true">
      <circle cx="6" cy="6" r="6" fill={color} />
    </svg>
  );
}

const candidateName = (c: GroupCandidate) =>
  c.is_self
    ? c.display_name === ""
      ? "Tú"
      : `${c.display_name} (tú)`
    : c.display_name || "Sin nombre";

function GroupsScreen({
  onStats,
  onCompare,
}: {
  /** Abre «Estadísticas del grupo» (#145). */
  onStats: (groupId: number) => void;
  /** Abre «Comparar grupos» (#121). */
  onCompare: () => void;
}) {
  const [view, setView] = useState<GroupsView | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Grupo que se está editando; `"new"` = uno nuevo.
  const [editing, setEditing] = useState<number | "new" | null>(null);

  const load = useCallback(() => {
    athleteGroups()
      .then(setView)
      .catch((err: unknown) => setError(String(err)));
  }, []);
  useEffect(load, [load]);

  const run = (action: Promise<unknown>, after?: () => void) => {
    setError(null);
    action
      .then(() => {
        after?.();
        load();
      })
      .catch((err: unknown) => setError(String(err)));
  };

  const firstFreeColor = () => {
    const used = new Set(view?.groups.map((g) => g.color) ?? []);
    return GROUP_COLORS.find((c) => !used.has(c)) ?? GROUP_COLORS[0];
  };

  return (
    <>
      <PageHeader
        title="Grupos"
        subtitle="Organiza a tus atletas. Uno puede estar en varios grupos."
        actions={
          <div className="row">
            {(view?.groups.length ?? 0) >= 2 && (
              <button type="button" className="btn" onClick={onCompare}>
                Comparar grupos
              </button>
            )}
            {editing !== "new" && (
              <button type="button" className="btn btn-primary" onClick={() => setEditing("new")}>
                Nuevo grupo
              </button>
            )}
          </div>
        }
      />
      {error !== null && <Notice kind="error">{error}</Notice>}

      {editing === "new" && (
        <div className="card">
          <GroupForm
            initial={{ name: "", description: "", color: firstFreeColor() }}
            submitLabel="Crear grupo"
            onCancel={() => setEditing(null)}
            onSubmit={(fields) => run(createAthleteGroup(fields), () => setEditing(null))}
          />
        </div>
      )}

      {view !== null && view.groups.length === 0 && editing !== "new" && (
        <div className="card">
          <EmptyState icon={<GroupIcon size={40} />} title="Aún no hay grupos">
            <p>
              Por ejemplo, «Juveniles» o «Equipo de relevos». Luego podrás ver sus estadísticas.
            </p>
          </EmptyState>
        </div>
      )}

      {view !== null && view.groups.length > 0 && (
        <div className="group-cards">
          {view.groups.map((group) =>
            editing === group.id ? (
              <div className="card" key={group.id}>
                <GroupForm
                  initial={group}
                  submitLabel="Guardar"
                  onCancel={() => setEditing(null)}
                  onSubmit={(fields) =>
                    run(updateAthleteGroup(group.id, fields), () => setEditing(null))
                  }
                  onDelete={() => run(deleteAthleteGroup(group.id), () => setEditing(null))}
                />
              </div>
            ) : (
              <GroupCard
                key={group.id}
                group={group}
                candidates={view.candidates}
                onEdit={() => setEditing(group.id)}
                onStats={() => onStats(group.id)}
                onMember={(runnerId, member) =>
                  run(setAthleteGroupMember(group.id, runnerId, member))
                }
              />
            ),
          )}
        </div>
      )}
    </>
  );
}

function GroupCard({
  group,
  candidates,
  onEdit,
  onStats,
  onMember,
}: {
  group: GroupInfo;
  candidates: GroupCandidate[];
  onEdit: () => void;
  onStats: () => void;
  onMember: (runnerId: string, member: boolean) => void;
}) {
  const known = new Set(candidates.map((c) => c.runner_id));
  // Miembros de los que ya no hay paquetes: siguen en el grupo.
  const missing = group.members.filter((id) => !known.has(id)).length;
  return (
    <section className="card group-card" aria-label={group.name}>
      <div className="group-card-head">
        <GroupDot color={group.color} size={14} />
        <h3>{group.name}</h3>
        <button
          type="button"
          className="btn btn-ghost btn-icon"
          onClick={onEdit}
          aria-label={`Editar ${group.name}`}
          title="Editar"
        >
          <PencilIcon />
        </button>
      </div>
      {group.description !== "" && <p className="small muted">{group.description}</p>}
      <fieldset className="group-members">
        <legend className="field-label">
          {group.members.length === 1 ? "1 miembro" : `${group.members.length} miembros`}
        </legend>
        {candidates.length === 0 && (
          <p className="small muted">Aún no ha llegado nada de tus atletas.</p>
        )}
        {candidates.map((c) => (
          <label key={c.runner_id} className="check">
            <input
              type="checkbox"
              checked={group.members.includes(c.runner_id)}
              onChange={(e) => onMember(c.runner_id, e.target.checked)}
            />
            {candidateName(c)}
          </label>
        ))}
        {missing > 0 && (
          <p className="small muted">
            {missing === 1
              ? "Y 1 atleta del que ya no hay carreras."
              : `Y ${missing} atletas de los que ya no hay carreras.`}
          </p>
        )}
      </fieldset>
      <div className="row">
        <button
          type="button"
          className="btn"
          onClick={onStats}
          disabled={group.members.length === 0}
        >
          <ChartIcon size={16} />
          Estadísticas del grupo
        </button>
      </div>
    </section>
  );
}

function GroupForm({
  initial,
  submitLabel,
  onSubmit,
  onCancel,
  onDelete,
}: {
  initial: GroupFields;
  submitLabel: string;
  onSubmit: (fields: GroupFields) => void;
  onCancel: () => void;
  /** Solo al editar uno que ya existe. */
  onDelete?: () => void;
}) {
  const [fields, setFields] = useState<GroupFields>({
    name: initial.name,
    description: initial.description,
    color: initial.color,
  });
  const [confirming, setConfirming] = useState(false);
  const empty = fields.name.trim() === "";
  return (
    <form
      className="group-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (!empty) onSubmit(fields);
      }}
    >
      <label className="field">
        <span className="field-label">Nombre</span>
        <input
          className="input"
          value={fields.name}
          autoFocus
          onChange={(e) => setFields({ ...fields, name: e.target.value })}
        />
      </label>
      <label className="field">
        <span className="field-label">Descripción</span>
        <input
          className="input"
          value={fields.description}
          placeholder="Opcional"
          onChange={(e) => setFields({ ...fields, description: e.target.value })}
        />
      </label>
      <div className="field">
        <span className="field-label" id="group-color">
          Color
        </span>
        <div className="swatches" role="radiogroup" aria-labelledby="group-color">
          {GROUP_COLORS.map((color, i) => (
            <label key={color} className="swatch" title={`Color ${i + 1}`}>
              <input
                type="radio"
                name="group-color"
                checked={fields.color === color}
                onChange={() => setFields({ ...fields, color })}
                aria-label={`Color ${i + 1}`}
              />
              <GroupDot color={color} size={20} />
            </label>
          ))}
        </div>
      </div>
      <div className="row">
        <button type="submit" className="btn btn-primary" disabled={empty}>
          {submitLabel}
        </button>
        <button type="button" className="btn btn-ghost" onClick={onCancel}>
          Cancelar
        </button>
        {onDelete !== undefined &&
          (confirming ? (
            <>
              <span className="small muted">Sus atletas y sus carreras se quedan.</span>
              <button type="button" className="btn btn-danger" onClick={onDelete}>
                Borrar el grupo
              </button>
            </>
          ) : (
            <button type="button" className="btn btn-ghost" onClick={() => setConfirming(true)}>
              Borrar…
            </button>
          ))}
      </div>
    </form>
  );
}

export default GroupsScreen;
