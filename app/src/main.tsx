import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";
import { invoke } from "@tauri-apps/api/core";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

if ("__TAURI_INTERNALS__" in window) {
  invoke<boolean>("bench_requested").then(async (on) => {
    if (!on) return;
    const [{ runBench }, { loadProvinces }] = await Promise.all([import("./bench"), import("./data")]);
    runBench((await loadProvinces()).id.length);
  });
}
