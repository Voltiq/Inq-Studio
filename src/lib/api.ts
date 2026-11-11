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
  /** Labels of the changes made, oldest first. */
  history: string[];
  /** Labels of undone changes, next first. */
  future: string[];
  /** The document carries digital signatures. */
  signed: boolean;
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

export interface Span {
  start: number;
  end: number;
  fontKey: string;
  fontName: string;
  family: string;
  bold: boolean;
  italic: boolean;
  size: number;
  em: number;
  color: [number, number, number];
}

export interface TextLine {
  id: number;
  /** "page", or "form n g" for text inside a Form XObject. */
  src: string;
  ops: number[];
  text: string;
  /** Stretches of one style. */
  spans: Span[];
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
  /** Character boundaries along the baseline from `origin`, in points (one more than characters). */
  chars: number[];
  /** The paragraph this line belongs to. */
  block: number;
}

export interface Block {
  id: number;
  lines: number[];
  text: string;
  align: "left" | "right" | "center" | "justify";
  width: number;
  leading: number;
  quad: Quad;
}

export interface PageText {
  revision: number;
  lines: TextLine[];
  blocks: Block[];
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
  /** Embedded by Inq Studio from an installed font. */
  added: boolean;
}

export interface EditResult {
  state: DocState;
  notice: string | null;
}

export interface LineEdit {
  text?: string;
  size?: number;
  color?: [number, number, number];
  font?: FontChoice;
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
  fill: [number, number, number] | null;
  opacity: number;
  width: number;
  /** For links: "https://…" or "page N". */
  target: string | null;
  removable: boolean;
  /** Its appearance can be redrawn after changes. */
  editable: boolean;
}

export type AnnotKind =
  | "highlight"
  | "underline"
  | "strikeout"
  | "note"
  | "ink"
  | "rect"
  | "ellipse"
  | "line"
  | "arrow"
  | "polygon"
  | "textbox"
  | "stamp"
  | "link";

export type Rect = [number, number, number, number];

export interface NewAnnot {
  kind: AnnotKind;
  quads?: Quad[];
  rect?: Rect;
  paths?: Point[][];
  points?: Point[];
  color: [number, number, number];
  fill?: [number, number, number];
  contents?: string;
  author?: string;
  width?: number;
  opacity?: number;
  size?: number;
  label?: string;
  uri?: string;
  page?: number;
}

export interface AnnotChange {
  color?: [number, number, number];
  /** null removes the fill. */
  fill?: [number, number, number] | null;
  contents?: string;
  opacity?: number;
  width?: number;
  dx?: number;
  dy?: number;
  uri?: string;
  page?: number;
}

export interface WidgetInfo {
  page: number;
  rect: Rect;
  onState: string | null;
  checked: boolean;
}

export interface FieldInfo {
  name: string;
  kind: "text" | "checkbox" | "radio" | "combo" | "list" | "button" | "signature" | "unknown";
  value: string;
  options: string[];
  multiline: boolean;
  readonly: boolean;
  required: boolean;
  maxLen: number | null;
  widgets: WidgetInfo[];
}

export interface ImageInfo {
  index: number;
  quad: Quad;
  bbox: Rect;
  width: number;
  height: number;
  inline: boolean;
  filter: string;
  colorSpace: string;
  bits: number;
  hasAlpha: boolean;
  dpi: number;
}

export interface Found {
  page: number;
  quads: Quad[];
}

export interface Bookmark {
  title: string;
  page: number | null;
  top?: number | null;
  open: boolean;
  children: Bookmark[];
}

export interface LabelRange {
  start: number;
  style: "D" | "r" | "R" | "a" | "A" | "";
  prefix: string;
  first: number;
}

export interface MarkStyle {
  font: FontChoice;
  size: number;
  color: [number, number, number];
  opacity: number;
}

export type Mark =
  | { kind: "watermark"; text: string; style: MarkStyle; angle: number; behind: boolean }
  | { kind: "headerFooter"; header: [string, string, string]; footer: [string, string, string]; style: MarkStyle; margin: number; firstNumber: number }
  | { kind: "bates"; prefix: string; suffix: string; start: number; digits: number; position: string; style: MarkStyle; margin: number };

export type SaveMode = "auto" | "full" | "incremental";

export interface FontCatalog {
  families: string[];
  standard: string[];
}

export const api = {
  open: (path: string, password?: string) => invoke<DocInfo>("doc_open", { path, password: password ?? null }),
  info: (id: number) => invoke<DocInfo>("doc_info", { id }),
  bytes: (id: number) => invoke<ArrayBuffer>("doc_bytes", { id }),
  close: (id: number) => invoke<void>("doc_close", { id }),
  save: (id: number, path?: string, mode?: SaveMode) =>
    invoke<DocState>("doc_save", { id, path: path ?? null, mode: mode ?? "auto" }),
  undo: (id: number) => invoke<DocState>("doc_undo", { id }),
  redo: (id: number) => invoke<DocState>("doc_redo", { id }),
  historyGo: (id: number, steps: number) => invoke<DocState>("doc_history_go", { id, steps }),
  setMeta: (id: number, meta: Meta) => invoke<DocState>("doc_set_meta", { id, meta }),
  pageText: (id: number, page: number) => invoke<PageText>("page_text", { id, page }),
  fonts: (id: number) => invoke<FontSummary[]>("doc_fonts", { id }),
  fontProgram: (id: number, key: string) => invoke<ArrayBuffer>("font_program", { id, key }),
  systemFonts: () => invoke<FontCatalog>("system_fonts"),
  editLines: (id: number, page: number, revision: number, lines: number[], edit: LineEdit) =>
    invoke<EditResult>("text_edit", { id, page, revision, lines, edit }),
  editBlock: (id: number, page: number, revision: number, block: number, text: string) =>
    invoke<EditResult>("block_edit", { id, page, revision, block, text }),
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
  updateAnnot: (id: number, page: number, index: number, change: AnnotChange) =>
    invoke<DocState>("annot_update", { id, page, index, change }),
  fields: (id: number) => invoke<FieldInfo[]>("form_fields", { id }),
  setField: (id: number, name: string, value: string) => invoke<DocState>("form_set", { id, name, value }),
  flatten: (id: number, scope: "forms" | "annotations" | "all") => invoke<EditResult>("doc_flatten", { id, scope }),
  images: (id: number, page: number) => invoke<ImageInfo[]>("images_list", { id, page }),
  insertImage: (id: number, page: number, path: string, at: Point | null) => invoke<DocState>("image_insert", { id, page, path, at }),
  placeImage: (id: number, page: number, index: number, bbox: Rect) => invoke<DocState>("image_place", { id, page, index, bbox }),
  deleteImage: (id: number, page: number, index: number) => invoke<DocState>("image_delete", { id, page, index }),
  replaceImage: (id: number, page: number, index: number, path: string) => invoke<DocState>("image_replace", { id, page, index, path }),
  extractImage: (id: number, page: number, index: number, path: string) => invoke<string>("image_extract", { id, page, index, path }),
  find: (id: number, query: string) => invoke<Found[]>("text_find", { id, query }),
  redact: (id: number, marks: { page: number; rects: Rect[] }[], fill: [number, number, number] | null) =>
    invoke<EditResult>("redact", { id, marks, fill }),
  pageMarks: (id: number, pages: number[], mark: Mark) => invoke<EditResult>("page_marks", { id, pages, mark }),
  labels: (id: number) => invoke<{ ranges: LabelRange[]; strings: string[] }>("labels_get", { id }),
  setLabels: (id: number, ranges: LabelRange[]) => invoke<DocState>("labels_set", { id, ranges }),
  outline: (id: number) => invoke<Bookmark[]>("outline_get", { id }),
  setOutline: (id: number, items: Bookmark[]) => invoke<DocState>("outline_set", { id, items }),
  crop: (id: number, pages: number[], area: Rect | null) => invoke<DocState>("pages_crop", { id, pages, area }),
  resizePages: (id: number, pages: number[], width: number, height: number) =>
    invoke<DocState>("pages_resize", { id, pages, width, height }),
  split: (id: number, ranges: number[][], folder: string) => invoke<string[]>("doc_split", { id, ranges, folder }),
};

/** Error text from a rejected command. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
