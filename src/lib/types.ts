export type ThemePreference = "system" | "light" | "dark";

export type Tool =
  | "select"
  | "hand"
  | "edit"
  | "text"
  | "image"
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
  | "link"
  | "redact"
  | "crop";

export type ShapeTool = "rect" | "ellipse" | "line" | "arrow" | "polygon";

export type ZoomMode = "fit-width" | "fit-page" | number;
