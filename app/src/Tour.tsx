// Recorrido guiado de la primera vez (#141, `docs/app.md`, "Recorrido guiado").
//
// Cada paso señala una parte de la pantalla, marcada con `data-tour="…"` en `App.tsx`. Todo se
// dibuja en un SVG a pantalla completa: el velo con el hueco del elemento señalado, el marco, el
// bocadillo (un `foreignObject`) y su pico. La posición va en atributos SVG (`x`, `y`, `width`,
// `height`, `points`), no en estilos: la CSP no deja estilos en línea.
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { setTourSeen, tourSeen } from "./api";
import { HelpPageId, helpTitle } from "./help/pages";

interface TourStep {
  /** Valor de `data-tour` de lo que señala. */
  target: string;
  title: string;
  text: string;
  /** Su página de ayuda. */
  help: HelpPageId;
  /** Solo si entrena. */
  coachOnly?: boolean;
}

/** Los pasos, en orden. */
const STEPS: TourStep[] = [
  {
    target: "import",
    title: "Importa tu primera carrera",
    text:
      "Empieza aquí: el .spl de WinSplits y, si lo tienes, el FIT de tu reloj. También puedes " +
      "importar una carpeta con toda una temporada.",
    help: "importar",
  },
  {
    target: "races",
    title: "Mis carreras",
    text:
      "Todas tus carreras, de la más reciente a la más antigua, con su tiempo perdido. El número " +
      "de al lado dice cuántos errores te quedan por revisar.",
    help: "mis-carreras",
  },
  {
    target: "content",
    title: "Una carrera y sus pestañas",
    text:
      "Al abrir una carrera, aquí verás sus pestañas: Resumen (cuánto has perdido y dónde), " +
      "Tramos (confirma los errores y di de qué tipo fueron), Mapa, Frente al grupo y Análisis.",
    help: "carrera",
  },
  {
    target: "history",
    title: "Estadísticas",
    text:
      "Todas tus carreras juntas: dónde fallas, cómo evolucionas y qué errores se repiten. " +
      "Con varias carreras empiezan a salir los patrones.",
    help: "estadisticas",
  },
  {
    target: "athletes",
    title: "Atletas",
    text:
      "Como entrenas, aquí eliges a un atleta para ver sus carreras en solo lectura, los " +
      "comparas entre sí y los organizas en grupos.",
    help: "atletas",
    coachOnly: true,
  },
  {
    target: "help",
    title: "Ayuda",
    text:
      "En cada pantalla, este botón (o la tecla F1) abre la ayuda de lo que estás viendo. " +
      "Desde la portada de la Ayuda puedes repetir este recorrido.",
    help: "indice",
  },
];

/** Margen con el borde de la ventana, separación con lo señalado y tamaño del pico (px). */
const MARGIN = 16;
const GAP = 14;
const ARROW = 9;
/** Ancho del bocadillo: el mismo que `.tour-bubble` en `components.css`. */
const WIDTH = 320;
/** Alto mientras no se ha medido. */
const START_HEIGHT = 200;
/** Holgura del hueco alrededor de lo señalado. */
const PAD = 4;

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Dónde va el bocadillo y su pico (`null`: sin pico, centrado sobre lo señalado). */
interface Placement {
  x: number;
  y: number;
  arrow: string | null;
}

const clamp = (v: number, min: number, max: number) => Math.max(min, Math.min(v, max));

/**
 * Coloca el bocadillo junto a lo señalado: a la derecha si cabe (la barra lateral), si no
 * debajo, si no encima y, si no cabe en ningún lado (lo señalado ocupa casi todo), centrado
 * encima de él. Siempre dentro de la ventana.
 */
function place(target: Box | null, vw: number, vh: number, height: number): Placement {
  const maxX = Math.max(MARGIN, vw - WIDTH - MARGIN);
  const maxY = Math.max(MARGIN, vh - height - MARGIN);
  if (target === null) {
    return {
      x: clamp((vw - WIDTH) / 2, MARGIN, maxX),
      y: clamp((vh - height) / 2, MARGIN, maxY),
      arrow: null,
    };
  }
  const right = target.x + target.width;
  const bottom = target.y + target.height;
  const cx = target.x + target.width / 2;
  const cy = target.y + target.height / 2;
  // El pico no llega a las esquinas redondeadas del bocadillo.
  const inset = 2 * ARROW;
  if (target.width < vw / 2 && right + GAP + WIDTH + MARGIN <= vw) {
    const x = right + GAP;
    const y = clamp(cy - height / 2, MARGIN, maxY);
    const ay = clamp(cy, y + inset, y + height - inset);
    return { x, y, arrow: `${x + 1},${ay - ARROW} ${x - ARROW},${ay} ${x + 1},${ay + ARROW}` };
  }
  if (bottom + GAP + height + MARGIN <= vh) {
    const x = clamp(cx - WIDTH / 2, MARGIN, maxX);
    const y = bottom + GAP;
    const ax = clamp(cx, x + inset, x + WIDTH - inset);
    return { x, y, arrow: `${ax - ARROW},${y + 1} ${ax},${y - ARROW} ${ax + ARROW},${y + 1}` };
  }
  if (target.y - GAP - height >= MARGIN) {
    const x = clamp(cx - WIDTH / 2, MARGIN, maxX);
    const y = target.y - GAP - height;
    const end = y + height;
    const ax = clamp(cx, x + inset, x + WIDTH - inset);
    return {
      x,
      y,
      arrow: `${ax - ARROW},${end - 1} ${ax},${end + ARROW} ${ax + ARROW},${end - 1}`,
    };
  }
  return {
    x: clamp(cx - WIDTH / 2, MARGIN, maxX),
    y: clamp(cy - height / 2, MARGIN, maxY),
    arrow: null,
  };
}

/**
 * Lo señalado por el paso, recortado a la ventana; `null` si no está o no se ve. Con `reveal`,
 * antes lo trae a la vista (p. ej. si la barra lateral está desplazada).
 */
function measure(target: string, vw: number, vh: number, reveal: boolean): Box | null {
  const el = document.querySelector(`[data-tour="${target}"]`);
  if (el === null) return null;
  if (reveal) el.scrollIntoView({ block: "nearest", inline: "nearest" });
  const r = el.getBoundingClientRect();
  const x = Math.max(0, r.left - PAD);
  const y = Math.max(0, r.top - PAD);
  const width = Math.min(vw, r.right + PAD) - x;
  const height = Math.min(vh, r.bottom + PAD) - y;
  return width > 0 && height > 0 ? { x, y, width, height } : null;
}

/**
 * Si toca enseñar el recorrido: cuando ya se ha elegido cómo se usa la app (`ready`) y aún no se
 * ha visto. Cerrarlo, al terminar o al saltarlo, lo guarda como visto. `start` lo repite.
 */
export function useTour(ready: boolean, onError: (err: unknown) => void) {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (!ready) return;
    let live = true;
    tourSeen()
      .then((seen) => {
        if (live && !seen) setOpen(true);
      })
      .catch(onError);
    return () => {
      live = false;
    };
  }, [ready, onError]);
  const start = useCallback(() => setOpen(true), []);
  const close = useCallback(() => {
    setOpen(false);
    setTourSeen(true).catch(onError);
  }, [onError]);
  return { open, start, close };
}

/** Selector de lo que se puede enfocar dentro del bocadillo, para no salir de él con Tab. */
const FOCUSABLE = "button:not(:disabled), [href], [tabindex]:not([tabindex='-1'])";

/** El recorrido: un cuadro modal que se mueve de una parte de la pantalla a otra. */
function Tour({
  coach,
  onClose,
  onHelp,
}: {
  /** Si entrena: añade el paso de Atletas. */
  coach: boolean;
  /** Termina o se salta. */
  onClose: () => void;
  /** Abre una página de ayuda (y cierra el recorrido). */
  onHelp: (page: HelpPageId) => void;
}) {
  const steps = STEPS.filter((s) => coach || s.coachOnly !== true);
  const [picked, setIndex] = useState(0);
  // Si deja de entrenar a mitad, sobra un paso.
  const index = Math.min(picked, steps.length - 1);
  const step = steps[index];
  const last = index === steps.length - 1;
  const [viewport, setViewport] = useState({
    width: window.innerWidth,
    height: window.innerHeight,
  });
  const [target, setTarget] = useState<Box | null>(null);
  const [height, setHeight] = useState(START_HEIGHT);
  const bubble = useRef<HTMLDivElement>(null);
  const primary = useRef<HTMLButtonElement>(null);

  const next = () => (last ? onClose() : setIndex(index + 1));
  const back = () => setIndex(Math.max(0, index - 1));

  // Lo señalado se vuelve a medir al cambiar de paso, de tamaño de ventana o al desplazarse.
  useLayoutEffect(() => {
    const update = (reveal = false) => {
      const vw = window.innerWidth;
      const vh = window.innerHeight;
      setViewport({ width: vw, height: vh });
      setTarget(measure(step.target, vw, vh, reveal));
    };
    update(true);
    const el = document.querySelector(`[data-tour="${step.target}"]`);
    const follow = () => update();
    const observer = new ResizeObserver(follow);
    if (el !== null) observer.observe(el);
    window.addEventListener("resize", follow);
    window.addEventListener("scroll", follow, true);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", follow);
      window.removeEventListener("scroll", follow, true);
    };
  }, [step.target]);

  // El alto del bocadillo depende del texto: se mide para colocarlo.
  useLayoutEffect(() => {
    const el = bubble.current;
    if (el === null) return;
    const update = () => setHeight(Math.ceil(el.offsetHeight));
    update();
    const observer = new ResizeObserver(update);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // Al abrir, el foco va a «Siguiente»; al cerrar, vuelve a donde estaba (si sigue ahí).
  useEffect(() => {
    const before = document.activeElement;
    primary.current?.focus();
    return () => {
      if (before instanceof HTMLElement && before.isConnected) before.focus();
    };
  }, []);

  // Si al cambiar de paso el foco se queda fuera (p. ej. en «Anterior», ya desactivado), a
  // «Siguiente».
  useEffect(() => {
    const active = document.activeElement;
    const inside =
      active instanceof HTMLElement &&
      bubble.current?.contains(active) === true &&
      !(active instanceof HTMLButtonElement && active.disabled);
    if (!inside) primary.current?.focus();
  }, [index]);

  // Esc salta el recorrido, F1 abre la ayuda del paso y Tab no sale del bocadillo.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "F1") {
        e.preventDefault();
        onHelp(step.help);
      } else if (e.key === "Tab" && bubble.current !== null) {
        const items = Array.from(bubble.current.querySelectorAll<HTMLElement>(FOCUSABLE));
        if (items.length === 0) return;
        const first = items[0];
        const end = items[items.length - 1];
        const active = document.activeElement;
        const outside = !(active instanceof Node) || !bubble.current.contains(active);
        if (outside || (e.shiftKey && active === first)) {
          e.preventDefault();
          (e.shiftKey ? end : first).focus();
        } else if (!e.shiftKey && active === end) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, onHelp, step.help]);

  const { width: vw, height: vh } = viewport;
  const at = place(target, vw, vh, height);
  return (
    <div className="tour">
      <svg className="tour-layer" width={vw} height={vh} viewBox={`0 0 ${vw} ${vh}`}>
        <defs>
          <mask id="tour-spotlight">
            <rect width={vw} height={vh} fill="white" />
            {target !== null && (
              <rect
                x={target.x}
                y={target.y}
                width={target.width}
                height={target.height}
                rx={8}
                fill="black"
              />
            )}
          </mask>
        </defs>
        <rect className="tour-veil" width={vw} height={vh} mask="url(#tour-spotlight)" />
        {target !== null && (
          <rect
            className="tour-ring"
            x={target.x}
            y={target.y}
            width={target.width}
            height={target.height}
            rx={8}
          />
        )}
        <foreignObject x={at.x} y={at.y} width={WIDTH} height={height}>
          <div
            ref={bubble}
            className="tour-bubble"
            role="dialog"
            aria-modal="true"
            aria-labelledby="tour-title"
            aria-describedby="tour-text"
          >
            <div className="tour-body" aria-live="polite">
              <span className="eyebrow">
                Paso {index + 1} de {steps.length}
              </span>
              <h2 id="tour-title" className="tour-title">
                {step.title}
              </h2>
              <p id="tour-text" className="tour-text">
                {step.text}
              </p>
            </div>
            <button type="button" className="btn-link tour-help" onClick={() => onHelp(step.help)}>
              Más en la ayuda: {helpTitle(step.help)}
            </button>
            <div className="tour-actions">
              <button type="button" className="btn btn-ghost" onClick={onClose}>
                Saltar
              </button>
              <span className="tour-spacer" />
              <button type="button" className="btn" onClick={back} disabled={index === 0}>
                Anterior
              </button>
              <button ref={primary} type="button" className="btn btn-primary" onClick={next}>
                {last ? "Terminar" : "Siguiente"}
              </button>
            </div>
          </div>
        </foreignObject>
        {/* El pico tapa el borde del bocadillo: relleno sin borde y, encima, sus dos lados. */}
        {at.arrow !== null && <polygon className="tour-arrow" points={at.arrow} />}
        {at.arrow !== null && <polyline className="tour-arrow-edge" points={at.arrow} />}
      </svg>
    </div>
  );
}

export default Tour;
