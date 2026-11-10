import { invoke } from "@tauri-apps/api/core";

// Shapes returned by the Rust side (src-tauri/src/store.rs).

export interface DocState {
  id: number;
  name: string;
  path: string | null;
  revision: number;
  pages: number;
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
}

export interface Meta {
  title: string;
  author: string;
  subject: string;
  keywords: string;
  creator: string;
  producer: string;
  created: string;
  modified: string;
}

export interface DocInfo {
  state: DocState;
  version: string;
  wasEncrypted: boolean;
  meta: Meta;
  fileSize: number;
}

export type Point = [number, number];
export type Quad = [Point, Point, Point, Point];

export interface TextLine {
  id: number;
  ops: number[];
  text: string;
  fontKey: string;
  fontName: string;
  family: string;
  bold: boolean;
  italic: boolean;
  /** Tf size. */
  size: number;
  /** Size as drawn, in points. */
  em: number;
  /** User-space corners: bottom-left, bottom-right, top-right, top-left. */
  quad: Quad;
  origin: Point;
  angle: number;
  color: [number, number, number];
  invisible: boolean;
  editable: boolean;
  note: string | null;
}

export interface PageText {
  revision: number;
  lines: TextLine[];
  rotation: number;
}

export interface FontSummary {
  key: string;
  name: string;
  rawName: string;
  family: string;
  style: string;
  kind: string;
  encoding: string;
  embedded: boolean;
  subset: boolean;
  program: "type1" | "truetype" | "cff" | "opentype" | null;
  glyphs: number;
  charset: string;
  hasToUnicode: boolean;
  extendable: boolean;
  editable: boolean;
  bold: boolean;
  italic: boolean;
  serif: boolean;
  fixed: boolean;
  pages: number[];
  systemMatch: string | null;
  previewable: boolean;
}

export interface EditResult {
  state: DocState;
  notice: string | null;
}

export interface LineEdit {
  text?: string;
  size?: number;
  color?: [number, number, number];
  dx?: number;
  dy?: number;
  delete?: boolean;
}

export type FontChoice =
  | { kind: "document"; key: string }
  | { kind: "system"; family: string; bold: boolean; italic: boolean }
  | { kind: "standard"; name: string };

export interface NewText {
  x: number;
  y: number;
  text: string;
  font: FontChoice;
  size: number;
  color: [number, number, number];
  lineHeight?: number;
}

export interface AnnotInfo {
  index: number;
  subtype: string;
  rect: [number, number, number, number];
  contents: string;
  author: string;
  color: [number, number, number] | null;
  removable: boolean;
}

export type AnnotKind = "highlight" | "underline" | "strikeout" | "note" | "ink" | "rect";

export interface NewAnnot {
  kind: AnnotKind;
  quads?: Quad[];
  rect?: [number, number, number, number];
  paths?: Point[][];
  color: [number, number, number];
  contents?: string;
  author?: string;
  width?: number;
  opacity?: number;
}

export interface FontCatalog {
  families: string[];
  standard: string[];
}

export const api = {
  open: (path: string, password?: string) => invoke<DocInfo>("doc_open", { path, password: password ?? null }),
  info: (id: number) => invoke<DocInfo>("doc_info", { id }),
  bytes: (id: number) => invoke<ArrayBuffer>("doc_bytes", { id }),
  close: (id: number) => invoke<void>("doc_close", { id }),
  save: (id: number, path?: string) => invoke<DocState>("doc_save", { id, path: path ?? null }),
  undo: (id: number) => invoke<DocState>("doc_undo", { id }),
  redo: (id: number) => invoke<DocState>("doc_redo", { id }),
  setMeta: (id: number, meta: Meta) => invoke<DocState>("doc_set_meta", { id, meta }),
  pageText: (id: number, page: number) => invoke<PageText>("page_text", { id, page }),
  fonts: (id: number) => invoke<FontSummary[]>("doc_fonts", { id }),
  fontProgram: (id: number, key: string) => invoke<ArrayBuffer>("font_program", { id, key }),
  systemFonts: () => invoke<FontCatalog>("system_fonts"),
  editLine: (id: number, page: number, revision: number, line: number, edit: LineEdit) =>
    invoke<EditResult>("text_edit", { id, page, revision, line, edit }),
  addText: (id: number, page: number, spec: NewText) => invoke<EditResult>("text_add", { id, page, spec }),
  rotate: (id: number, pages: number[], delta: number) => invoke<DocState>("pages_rotate", { id, pages, delta }),
  deletePages: (id: number, pages: number[]) => invoke<DocState>("pages_delete", { id, pages }),
  reorder: (id: number, order: number[]) => invoke<DocState>("pages_reorder", { id, order }),
  insertBlank: (id: number, at: number) => invoke<DocState>("page_insert_blank", { id, at }),
  duplicate: (id: number, index: number) => invoke<DocState>("page_duplicate", { id, index }),
  insertFile: (id: number, path: string, at: number) => invoke<EditResult>("pages_insert_file", { id, path, at }),
  extract: (id: number, pages: number[], path: string) => invoke<void>("pages_extract", { id, pages, path }),
  annots: (id: number, page: number) => invoke<AnnotInfo[]>("annots_list", { id, page }),
  addAnnot: (id: number, page: number, annot: NewAnnot) => invoke<DocState>("annot_add", { id, page, annot }),
  deleteAnnot: (id: number, page: number, index: number) => invoke<DocState>("annot_delete", { id, page, index }),
  fileExists: (path: string) => invoke<boolean>("file_exists", { path }),
  startupFiles: () => invoke<string[]>("startup_files"),
};

/** Error text from a rejected command. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
