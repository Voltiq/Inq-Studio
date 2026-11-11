import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { resolveTheme, setPageTheme } from "./lib/theme";
import { loadSettings } from "./lib/storage";
import "./styles/tokens.css";
import "./styles/components.css";
import "./styles/forms.css";
import "./styles/app.css";

// Colour the page before the first render, so it never flashes the wrong theme.
setPageTheme(resolveTheme(loadSettings().theme));

// In a plain browser during development there is no engine: use the stand-in.
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  (await import("./lib/mock")).installMock();
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
