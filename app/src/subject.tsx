// De quién hablan los textos de Estadísticas (#165): del propio usuario («tus tramos»), de un atleta
// que se ve en solo lectura («sus tramos») o de un grupo («fallan»). Los paneles son los mismos en
// las tres vistas; solo cambian las palabras, y cada panel las elige con `useSubject` en vez de
// llevar una copia por caso. Sin proveedor vale lo que diga `Viewer`: quien entrena viendo a otro
// es «atleta» y el resto, «propio». Las Estadísticas de un grupo ponen `"group"`.
import { createContext, useContext } from "react";
import { useViewer } from "./viewer";

export type SubjectKind = "self" | "athlete" | "group";

export interface Subject {
  kind: SubjectKind;
  /** «tus» / «sus»: delante de un plural («tus tramos», «sus tramos»). */
  your: "tus" | "sus";
  /** «tu» / «su»: delante de un singular («tu media», «su media»). */
  yourOne: "tu" | "su";
  /**
   * El texto que toca: el propio (segunda persona), el de un atleta (tercera del singular) o el de
   * un grupo (tercera del plural). Sin texto de grupo vale el del atleta.
   */
  pick: <T>(self: T, athlete: T, group?: T) => T;
}

export function subjectOf(kind: SubjectKind): Subject {
  return {
    kind,
    your: kind === "self" ? "tus" : "sus",
    yourOne: kind === "self" ? "tu" : "su",
    pick: (self, athlete, group) =>
      kind === "self" ? self : kind === "athlete" || group === undefined ? athlete : group,
  };
}

/** `null`: lo decide quién mira (`Viewer`). */
export const SubjectContext = createContext<SubjectKind | null>(null);

export function useSubject(): Subject {
  const kind = useContext(SubjectContext);
  const { runnerName } = useViewer();
  return subjectOf(kind ?? (runnerName === null ? "self" : "athlete"));
}
