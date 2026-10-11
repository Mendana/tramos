import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { UpdateStatus } from "./updates";

/**
 * «Acerca de Tramos» (ver `docs/app.md`, "Acerca de Tramos"): quién hace la app, su licencia, los
 * créditos y cómo informar de un fallo. Los destinos que la ventana puede abrir (el correo y el
 * repositorio) los limita `capabilities/default.json`.
 */
const REPO_URL = "https://github.com/Mendana/tramos";
const CONTACT_EMAIL = "diegodiazmenda@gmail.com";

/** El sistema operativo, a partir del `userAgent` de la ventana web (sin plugin de sistema). */
export function describeSystem(userAgent: string): string {
  const arch = /Win64|x64|x86_64|amd64/i.test(userAgent)
    ? " (64 bits)"
    : /arm64|aarch64/i.test(userAgent)
      ? " (ARM)"
      : "";
  if (/Windows NT 10\.0/.test(userAgent)) return `Windows 10 u 11${arch}`;
  if (/Windows/.test(userAgent)) return `Windows${arch}`;
  if (/Mac OS X/.test(userAgent)) return `macOS${arch}`;
  if (/Linux/.test(userAgent)) return `Linux${arch}`;
  return userAgent === "" ? "desconocido" : userAgent;
}

/** Los datos que van al informe: solo versión y sistema, nada de carreras. */
export function reportData(version: string | null, userAgent: string): string {
  return `Versión de Tramos: ${version ?? "desconocida"}\nSistema: ${describeSystem(userAgent)}`;
}

/** El enlace `mailto:` con el asunto y el cuerpo ya rellenos, y un hueco para contar qué pasó. */
export function reportMailto(version: string | null, userAgent: string): string {
  const subject = `Fallo en Tramos ${version ?? ""}`.trim();
  const body = `${reportData(version, userAgent)}\n\n¿Qué ha pasado? (qué hacías y qué esperabas):\n\n`;
  return `mailto:${CONTACT_EMAIL}?subject=${encodeURIComponent(subject)}&body=${encodeURIComponent(body)}`;
}

/** Contenido del apartado de Ajustes «Acerca de Tramos». */
export function AboutSection() {
  const [version, setVersion] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);

  const report = async () => {
    try {
      await openUrl(reportMailto(version, navigator.userAgent));
      setMessage("Se ha abierto tu correo con el mensaje listo. No se envía hasta que lo mandes.");
    } catch {
      setMessage(`No se ha podido abrir el correo. Copia los datos y escribe a ${CONTACT_EMAIL}.`);
    }
  };
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(reportData(version, navigator.userAgent));
      setMessage("Copiado. Pégalo en tu mensaje.");
    } catch {
      setMessage("No se ha podido copiar.");
    }
  };

  return (
    <>
      <UpdateStatus />
      <div className="field">
        <span className="field-label">Autor: Mendaña.</span>
        <span className="field-hint">
          Tramos es de código abierto, con licencia MIT.{" "}
          <a href={REPO_URL} target="_blank" rel="noopener noreferrer">
            Repositorio en GitHub
          </a>
          .
        </span>
      </div>
      <div className="field">
        <span className="field-label">Créditos</span>
        <span className="field-hint">
          Mapa: teselas de{" "}
          <a
            href="https://www.openstreetmap.org/copyright"
            target="_blank"
            rel="noopener noreferrer"
          >
            © OpenStreetMap contributors
          </a>
          . Splits: formato de WinSplits. App de escritorio: Tauri.
        </span>
      </div>
      <div className="field">
        <span className="field-label">Informar de un fallo</span>
        <div className="row">
          <button type="button" className="btn" onClick={() => void report()}>
            Informar de un fallo
          </button>
          <button type="button" className="btn" onClick={() => void copy()}>
            Copiar datos para el informe
          </button>
        </div>
        <span className="field-hint">
          Abre tu correo con un mensaje para {CONTACT_EMAIL} que lleva la versión y tu sistema. No
          adjunta carreras ni datos tuyos, y no se envía hasta que lo mandes tú.
        </span>
        {message !== null && (
          <span className="field-hint" role="status">
            {message}
          </span>
        )}
      </div>
    </>
  );
}
