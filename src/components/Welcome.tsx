import { FileText, FolderOpen, Highlighter, LayoutGrid, Type, X } from "lucide-react";
import { Button, IconButton } from "./ui/Button";
import { Shortcut } from "./ui/Feedback";
import type { RecentFile } from "../lib/storage";
import { MOD } from "../lib/utils";

function ago(t: number): string {
  const s = (Date.now() - t) / 1000;
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  if (s < 86400 * 7) return `${Math.floor(s / 86400)} d ago`;
  return new Date(t).toLocaleDateString();
}

export function Welcome({
  recent,
  onOpen,
  onOpenPath,
  onForget,
}: {
  recent: RecentFile[];
  onOpen: () => void;
  onOpenPath: (path: string) => void;
  onForget: (path: string) => void;
}) {
  return (
    <div className="welcome">
      <div className="welcome-inner">
        <div className="welcome-hero">
          <img src="/app-icon.svg" alt="" />
          <div>
            <h1 className="welcome-title">Inq Studio</h1>
            <p className="welcome-sub">Read, edit and organise PDFs — and write in the fonts they already use.</p>
          </div>
        </div>

        <div className="dropzone">
          <span className="icon-tile icon-tile-large" aria-hidden="true">
            <FolderOpen size={20} />
          </span>
          <div className="dropzone-text">
            <p className="dropzone-title">Open a PDF</p>
            <p className="dropzone-sub">Or drop PDF files anywhere on this window.</p>
          </div>
          <Button appearance="primary" iconBefore={FolderOpen} shortcut={`${MOD}+O`} onClick={onOpen}>
            Open…
          </Button>
        </div>

        {recent.length > 0 && (
          <section className="recent">
            <span className="eyebrow">Recent</span>
            <div className="recent-list">
              {recent.map((r) => (
                <div key={r.path} style={{ position: "relative" }}>
                  <button type="button" className="recent-item" onClick={() => onOpenPath(r.path)}>
                    <FileText size={16} className="is-muted" aria-hidden="true" />
                    <span style={{ minWidth: 0 }}>
                      <span className="recent-name" style={{ display: "block" }}>
                        {r.name}
                      </span>
                      <span className="recent-path" style={{ display: "block" }}>
                        {r.path}
                      </span>
                    </span>
                    <span className="recent-when" style={{ marginRight: "2rem" }}>
                      {r.pages ? `${r.pages} p · ` : ""}
                      {ago(r.openedAt)}
                    </span>
                  </button>
                  <IconButton
                    icon={X}
                    label="Remove from list"
                    spacing="compact"
                    style={{ position: "absolute", right: 8, top: "50%", transform: "translateY(-50%)" }}
                    onClick={() => onForget(r.path)}
                  />
                </div>
              ))}
            </div>
          </section>
        )}

        <div className="features">
          <div className="feature">
            <span className="icon-tile" aria-hidden="true">
              <Type size={16} />
            </span>
            <p className="feature-title">Edit in the original font</p>
            <p className="feature-body">
              Text is re-encoded into the document’s own embedded fonts. Missing glyphs come from the same typeface on
              your system.
            </p>
          </div>
          <div className="feature">
            <span className="icon-tile" aria-hidden="true">
              <Highlighter size={16} />
            </span>
            <p className="feature-title">Mark up</p>
            <p className="feature-body">Highlights, underlines, notes, drawings and shapes that every reader shows.</p>
          </div>
          <div className="feature">
            <span className="icon-tile" aria-hidden="true">
              <LayoutGrid size={16} />
            </span>
            <p className="feature-title">Organise pages</p>
            <p className="feature-body">Reorder by dragging, rotate, duplicate, insert, merge and extract pages.</p>
          </div>
        </div>

        <div className="shortcuts">
          <span>
            <Shortcut keys="E" /> edit text
          </span>
          <span>
            <Shortcut keys="T" /> add text
          </span>
          <span>
            <Shortcut keys="M" /> highlight
          </span>
          <span>
            <Shortcut keys={`${MOD}+F`} /> find
          </span>
          <span>
            <Shortcut keys={`${MOD}+Z`} /> undo
          </span>
        </div>
      </div>
    </div>
  );
}
