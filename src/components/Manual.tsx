import { Fragment, useMemo, useRef, useState, type ReactNode } from "react";
import { BookOpen } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Modal } from "./ui/Modal";
import { currentLanguage, t } from "../lib/i18n";
import { cx } from "../lib/utils";
import english from "../../docs/GUIDE.md?raw";
import german from "../../docs/ANLEITUNG.md?raw";
import spanish from "../../docs/GUIA.md?raw";
import french from "../../docs/MANUEL.md?raw";

const MANUALS = { en: english, de: german, es: spanish, fr: french };

/*
 * The manual inside the app. It shows the same file that is in the
 * repository (docs/GUIDE.md and its translations), so the two cannot drift
 * apart. Only the Markdown those files use is understood.
 */

export type Block =
  | { kind: "heading"; level: number; text: string; id: string }
  | { kind: "paragraph"; text: string }
  | { kind: "list"; ordered: boolean; items: string[] }
  | { kind: "table"; rows: string[][] };

export function parseManual(source: string): Block[] {
  const blocks: Block[] = [];
  const lines = source.replace(/\r/g, "").split("\n");
  let i = 0;
  let headings = 0;
  const cells = (line: string) =>
    line
      .trim()
      .replace(/^\||\|$/g, "")
      .split("|")
      .map((c) => c.trim());
  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      i++;
      continue;
    }
    const heading = /^(#{1,3})\s+(.*)$/.exec(line);
    if (heading) {
      blocks.push({ kind: "heading", level: heading[1].length, text: heading[2], id: `manual-${headings++}` });
      i++;
      continue;
    }
    const item = /^(\s*)(-|\d+\.)\s+(.*)$/.exec(line);
    if (item) {
      const ordered = item[2] !== "-";
      const items: string[] = [];
      while (i < lines.length) {
        const next = /^(-|\d+\.)\s+(.*)$/.exec(lines[i]);
        if (next) items.push(next[2]);
        else if (/^\s+\S/.test(lines[i]) && items.length) items[items.length - 1] += " " + lines[i].trim();
        else break;
        i++;
      }
      blocks.push({ kind: "list", ordered, items });
      continue;
    }
    if (line.trim().startsWith("|")) {
      const rows: string[][] = [];
      while (i < lines.length && lines[i].trim().startsWith("|")) {
        if (!/^[\s|:-]+$/.test(lines[i])) rows.push(cells(lines[i]));
        i++;
      }
      blocks.push({ kind: "table", rows: rows.filter((r) => r.some((c) => c)) });
      continue;
    }
    const text: string[] = [];
    while (i < lines.length && lines[i].trim() && !/^(#{1,3}\s|-\s|\d+\.\s|\|)/.test(lines[i])) text.push(lines[i++].trim());
    blocks.push({ kind: "paragraph", text: text.join(" ") });
  }
  return blocks;
}

/** Bold, italics, code and links inside a line. */
function inline(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  const pattern = /\*\*(.+?)\*\*|\*(.+?)\*|`([^`]+)`|\[([^\]]+)\]\(([^)]+)\)/g;
  let last = 0;
  let m: RegExpExecArray | null;
  let key = 0;
  while ((m = pattern.exec(text))) {
    if (m.index > last) out.push(text.slice(last, m.index));
    if (m[1] != null) out.push(<strong key={key++}>{inline(m[1])}</strong>);
    else if (m[2] != null) out.push(<em key={key++}>{inline(m[2])}</em>);
    else if (m[3] != null) out.push(<kbd key={key++} className="manual-key">{m[3]}</kbd>);
    else if (/^https?:/.test(m[5])) {
      const url = m[5];
      out.push(
        <a
          key={key++}
          href={url}
          onClick={(e) => {
            e.preventDefault();
            openUrl(url).catch(() => {});
          }}
        >
          {m[4]}
        </a>,
      );
    } else out.push(<Fragment key={key++}>{m[4]}</Fragment>);
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

/** Parsed Markdown as elements; headings of the first level are left to the caller. */
export function MarkdownBlocks({ blocks }: { blocks: Block[] }) {
  return (
    <>
      {blocks.map((b, i) => {
        if (b.kind === "heading") {
          if (b.level === 1) return null;
          return b.level === 2 ? (
            <h3 key={i} id={b.id} className="manual-h2">
              {inline(b.text)}
            </h3>
          ) : (
            <h4 key={i} id={b.id} className="manual-h3">
              {inline(b.text)}
            </h4>
          );
        }
        if (b.kind === "paragraph") return <p key={i}>{inline(b.text)}</p>;
        if (b.kind === "list") {
          const List = b.ordered ? "ol" : "ul";
          return (
            <List key={i}>
              {b.items.map((item, n) => (
                <li key={n}>{inline(item)}</li>
              ))}
            </List>
          );
        }
        return (
          <table key={i} className="manual-table">
            <tbody>
              {b.rows.map((row, n) => (
                <tr key={n}>
                  {row.map((cell, k) => (
                    <td key={k}>{inline(cell)}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        );
      })}
    </>
  );
}

export function ManualDialog({ onClose }: { onClose: () => void }) {
  // The pointer to the other language's file means nothing inside the app.
  const blocks = useMemo(() => parseManual(MANUALS[currentLanguage()].replace(/\s*\*[^*]*\]\([^)]*\.md\)[^*]*\*/, "")), []);
  const chapters = blocks.filter((b): b is Extract<Block, { kind: "heading" }> => b.kind === "heading" && b.level === 2);
  const [current, setCurrent] = useState(chapters[0]?.id ?? "");
  const body = useRef<HTMLDivElement>(null);
  const title = blocks.find((b) => b.kind === "heading" && b.level === 1);

  const onScroll = () => {
    const el = body.current;
    if (!el) return;
    const top = el.getBoundingClientRect().top;
    let now = chapters[0]?.id ?? "";
    for (const c of chapters) {
      const h = el.querySelector(`#${c.id}`);
      if (h && h.getBoundingClientRect().top - top < 48) now = c.id;
    }
    setCurrent(now);
  };

  return (
    <Modal title={title?.kind === "heading" ? title.text : t("Manual")} titleIcon={<BookOpen size={18} aria-hidden="true" />} width="large" className="manual" onClose={onClose}>
      <div className="manual-layout">
        <nav className="manual-contents" aria-label={t("Contents")}>
          {chapters.map((c) => (
            <button
              key={c.id}
              type="button"
              className={cx("manual-chapter", c.id === current && "is-current")}
              onClick={() => {
                body.current?.querySelector(`#${c.id}`)?.scrollIntoView({ block: "start" });
                setCurrent(c.id);
              }}
            >
              {c.text}
            </button>
          ))}
        </nav>
        <div ref={body} className="manual-text" tabIndex={0} data-autofocus onScroll={onScroll}>
          <MarkdownBlocks blocks={blocks} />
        </div>
      </div>
    </Modal>
  );
}
