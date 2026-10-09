// Renderizador mínimo de Markdown para las páginas de ayuda (`docs/app.md`, "Ayuda").
//
// Solo entiende lo que usan las páginas: títulos (#, ##, ###), párrafos, listas de un nivel
// (- y 1.), citas (>, se pintan como «Ejemplo»), tablas con barras, **negrita**, *cursiva* o
// _cursiva_, `código` y [enlaces](destino). Construye elementos de React, nunca HTML: lo que no
// entiende sale como texto, así que no hay HTML crudo ni estilos en línea (CSP).
import { Fragment, ReactNode } from "react";

/** Prefijo de los documentos técnicos del repositorio: se abren en el navegador del sistema. */
export const DOCS_BASE = "https://github.com/Mendana/tramos/blob/main/";

type Block =
  | { kind: "heading"; level: 1 | 2 | 3; text: string }
  | { kind: "paragraph"; text: string }
  | { kind: "list"; ordered: boolean; items: string[] }
  | { kind: "quote"; paragraphs: string[] }
  | { kind: "table"; head: string[]; rows: string[][] };

const HEADING = /^(#{1,3})\s+(.*)$/;
const LIST_ITEM = /^\s*([-*]|\d+\.)\s+(.*)$/;
const TABLE_SEPARATOR = /^\|?\s*:?-{3,}/;

function startsBlock(line: string): boolean {
  return HEADING.test(line) || line.startsWith(">") || line.startsWith("|") || LIST_ITEM.test(line);
}

function tableCells(line: string): string[] {
  return line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((cell) => cell.trim());
}

/** Trocea el Markdown en bloques. */
export function parseMarkdown(source: string): Block[] {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (line.trim() === "") {
      i++;
      continue;
    }
    const heading = HEADING.exec(line);
    if (heading !== null) {
      const level = heading[1].length as 1 | 2 | 3;
      blocks.push({ kind: "heading", level, text: heading[2].trim() });
      i++;
      continue;
    }
    if (line.startsWith(">")) {
      const paragraphs: string[] = [];
      let current: string[] = [];
      while (i < lines.length && lines[i].startsWith(">")) {
        const text = lines[i].replace(/^>\s?/, "").trim();
        if (text === "") {
          if (current.length > 0) paragraphs.push(current.join(" "));
          current = [];
        } else {
          current.push(text);
        }
        i++;
      }
      if (current.length > 0) paragraphs.push(current.join(" "));
      blocks.push({ kind: "quote", paragraphs });
      continue;
    }
    if (line.startsWith("|")) {
      const rows: string[][] = [];
      while (i < lines.length && lines[i].startsWith("|")) {
        if (!TABLE_SEPARATOR.test(lines[i])) rows.push(tableCells(lines[i]));
        i++;
      }
      const [head = [], ...body] = rows;
      blocks.push({ kind: "table", head, rows: body });
      continue;
    }
    const item = LIST_ITEM.exec(line);
    if (item !== null) {
      const ordered = /\d/.test(item[1]);
      const items: string[] = [];
      while (i < lines.length) {
        const next = LIST_ITEM.exec(lines[i]);
        if (next !== null && /\d/.test(next[1]) === ordered) {
          items.push(next[2].trim());
        } else if (items.length > 0 && /^\s+\S/.test(lines[i])) {
          // Línea de continuación del elemento anterior.
          items[items.length - 1] += ` ${lines[i].trim()}`;
        } else {
          break;
        }
        i++;
      }
      blocks.push({ kind: "list", ordered, items });
      continue;
    }
    const text: string[] = [];
    while (
      i < lines.length &&
      lines[i].trim() !== "" &&
      (text.length === 0 || !startsBlock(lines[i]))
    ) {
      text.push(lines[i].trim());
      i++;
    }
    blocks.push({ kind: "paragraph", text: text.join(" ") });
  }
  return blocks;
}

/** El título (`# …`) de una página, o `null` si no lo tiene. */
export function markdownTitle(source: string): string | null {
  const first = parseMarkdown(source)[0];
  return first?.kind === "heading" && first.level === 1 ? first.text : null;
}

/** Destino de un enlace: otra página de ayuda, un documento técnico o nada que se pueda abrir. */
type LinkTarget = { kind: "page"; page: string } | { kind: "docs"; url: string } | null;

function linkTarget(href: string, isPage: (id: string) => boolean): LinkTarget {
  const page = /^([a-z0-9-]+)\.md$/.exec(href);
  if (page !== null) return isPage(page[1]) ? { kind: "page", page: page[1] } : null;
  if (href.startsWith(DOCS_BASE)) return { kind: "docs", url: href };
  return null;
}

// Código, negrita, enlace, cursiva con * y cursiva con _ (en ese orden de preferencia).
const INLINE =
  /`([^`]+)`|\*\*(.+?)\*\*|\[([^\]]+)\]\(([^)\s]+)\)|\*([^*\s][^*]*)\*|_([^_\s][^_]*)_/g;

interface InlineOptions {
  isPage: (id: string) => boolean;
  onOpenPage: (id: string) => void;
}

function renderInline(text: string, options: InlineOptions): ReactNode[] {
  const out: ReactNode[] = [];
  let last = 0;
  for (const match of text.matchAll(INLINE)) {
    const at = match.index ?? 0;
    if (at > last) out.push(text.slice(last, at));
    const key = out.length;
    const [, code, bold, linkText, href, em, em2] = match;
    if (code !== undefined) {
      out.push(<code key={key}>{code}</code>);
    } else if (bold !== undefined) {
      out.push(<strong key={key}>{renderInline(bold, options)}</strong>);
    } else if (linkText !== undefined && href !== undefined) {
      const content = renderInline(linkText, options);
      const target = linkTarget(href, options.isPage);
      if (target?.kind === "page") {
        out.push(
          <a
            key={key}
            href={`#ayuda-${target.page}`}
            onClick={(e) => {
              e.preventDefault();
              options.onOpenPage(target.page);
            }}
          >
            {content}
          </a>,
        );
      } else if (target?.kind === "docs") {
        // `target="_blank"`: tauri-plugin-opener lo abre en el navegador del sistema (los
        // permisos de la ventana solo dejan abrir el repositorio y OpenStreetMap).
        out.push(
          <a
            key={key}
            href={target.url}
            target="_blank"
            rel="noopener noreferrer"
            title="Se abre en el navegador"
          >
            {content}
          </a>,
        );
      } else {
        out.push(<Fragment key={key}>{content}</Fragment>);
      }
    } else {
      out.push(<em key={key}>{renderInline(em ?? em2 ?? "", options)}</em>);
    }
    last = at + match[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

/** Pinta una página de Markdown. Sin el título si `withoutTitle` (lo pinta la cabecera). */
export function Markdown({
  source,
  withoutTitle = false,
  isPage,
  onOpenPage,
}: {
  source: string;
  withoutTitle?: boolean;
  isPage: (id: string) => boolean;
  onOpenPage: (id: string) => void;
}) {
  const options = { isPage, onOpenPage };
  const inline = (text: string) => renderInline(text, options);
  const blocks = parseMarkdown(source);
  const shown =
    withoutTitle && blocks[0]?.kind === "heading" && blocks[0].level === 1
      ? blocks.slice(1)
      : blocks;
  return (
    <>
      {shown.map((block, i) => {
        switch (block.kind) {
          case "heading": {
            if (block.level === 1) return <h1 key={i}>{inline(block.text)}</h1>;
            if (block.level === 2) return <h2 key={i}>{inline(block.text)}</h2>;
            return <h3 key={i}>{inline(block.text)}</h3>;
          }
          case "paragraph":
            return <p key={i}>{inline(block.text)}</p>;
          case "list": {
            const items = block.items.map((item, j) => <li key={j}>{inline(item)}</li>);
            return block.ordered ? <ol key={i}>{items}</ol> : <ul key={i}>{items}</ul>;
          }
          case "quote":
            return (
              <aside key={i} className="help-example" aria-label="Ejemplo">
                <span className="help-example-label">Ejemplo</span>
                {block.paragraphs.map((p, j) => (
                  <p key={j}>{inline(p)}</p>
                ))}
              </aside>
            );
          case "table":
            return (
              <div key={i} className="table-wrap">
                <table className="table">
                  <thead>
                    <tr>
                      {block.head.map((cell, j) => (
                        <th key={j}>{inline(cell)}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {block.rows.map((row, j) => (
                      <tr key={j}>
                        {row.map((cell, k) => (
                          <td key={k}>{inline(cell)}</td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            );
        }
      })}
    </>
  );
}
