import { AppMode } from "./api";
import { ControlFlag } from "./ui";

/** Primera vez que se abre la app: ¿corredor o entrenadora? Se puede cambiar en Ajustes. */
function Welcome({ onChoose }: { onChoose: (mode: AppMode) => void }) {
  return (
    <div className="welcome">
      <div className="card welcome-card">
        <div className="brand">
          <ControlFlag />
          Tramos
        </div>
        <h1>¿Cómo vas a usar la app?</h1>
        <div className="welcome-options">
          <button type="button" className="welcome-option" onClick={() => onChoose("runner")}>
            <span className="strong">Soy corredor</span>
            <span className="small muted">
              Importo mis carreras, etiqueto mis errores y decido qué comparto con la entrenadora.
            </span>
          </button>
          <button type="button" className="welcome-option" onClick={() => onChoose("coach")}>
            <span className="strong">Soy la entrenadora</span>
            <span className="small muted">
              Veo las carreras que comparten los corredores, sin poder modificarlas.
            </span>
          </button>
        </div>
        <p className="small muted">Lo puedes cambiar después en Ajustes.</p>
      </div>
    </div>
  );
}

export default Welcome;
