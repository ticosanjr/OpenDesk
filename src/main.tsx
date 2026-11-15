import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/inter/400.css";
import "@fontsource/inter/500.css";
import "@fontsource/inter/600.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "./styles.css";
import App from "./App";
import { ErrorBoundary } from "./components/ui/error-boundary";

if (
  !("__TAURI_INTERNALS__" in window) &&
  "serviceWorker" in navigator
) {
  void navigator.serviceWorker.register("/sw.js").catch(() => {
    // PWA caching is optional; the app still works without it.
  });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);
