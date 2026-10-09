import { Role } from "./api";
import { ControlFlag } from "./ui";

const ROLES: { role: Role; title: string; text: string }[] = [
  {
    role: "runner",
    title: "Corro",
    text: "Importo mis carreras, etiqueto mis errores y decido qué comparto con quien me entrena.",
  },
  {
    role: "coach",
    title: "Entreno",
    text: "Veo las carreras que comparten mis atletas, sin poder modificarlas.",
  },
  {
    role: "both",
    title: "Las dos cosas",
    text: "Corro y además entreno a otros: mis carreras y las de mis atletas, a la vez.",
  },
];

/** Primera vez que se abre la app: ¿corre, entrena o las dos cosas? Se puede cambiar en Ajustes. */
function Welcome({ onChoose }: { onChoose: (role: Role) => void }) {
  return (
    <div className="welcome">
      <div className="card welcome-card">
        <div className="brand">
          <ControlFlag />
          Tramos
        </div>
        <h1>¿Cómo vas a usar la app?</h1>
        <div className="welcome-options">
          {ROLES.map((r) => (
            <button
              key={r.role}
              type="button"
              className="welcome-option"
              onClick={() => onChoose(r.role)}
            >
              <span className="strong">{r.title}</span>
              <span className="small muted">{r.text}</span>
            </button>
          ))}
        </div>
        <p className="small muted">Lo puedes cambiar después en Ajustes.</p>
      </div>
    </div>
  );
}

export default Welcome;
