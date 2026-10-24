// Map benchmark inside the desktop app's own webview (`CETERIS_BENCH=1 cargo run` in
// src-tauri). Measures frame times over a camera flight with county borders on, and the
// full cost of recolouring every county from the core: query, IPC, colouring, to screen.

import { invoke } from "@tauri-apps/api/core";
import type { Map as MlMap } from "maplibre-gl";

const log = (line: string) => invoke("bench_log", { line: `${performance.now().toFixed(0)} ${line}` });
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const frame = () => new Promise<number>((r) => requestAnimationFrame(r));
const RAMP = ["#fde0c5", "#f9b98a", "#f28f53", "#e0662b", "#b84a1b", "#8a3512", "#5c230b"];
const BREAKS = [50, 75, 100, 130, 160, 200];
const UPDATES = 20;

function summary(xs: number[]) {
  const s = [...xs].sort((a, b) => a - b);
  const q = (p: number) => +s[Math.min(s.length - 1, Math.floor(p * s.length))].toFixed(2);
  return { p50: q(0.5), p95: q(0.95), max: q(1) };
}

async function settle(m: MlMap) {
  await sleep(300);
  for (let i = 0; i < 200 && !m.areTilesLoaded(); i++) await sleep(50);
}

async function flight(m: MlMap) {
  const legs: [[number, number], number][] = [[[100, 30], 2.2], [[-80, 25], 1.8], [[10, 50], 4.5], [[45, 25], 3], [[-60, -10], 2.4], [[8, 48], 6]];
  const dt: number[] = [];
  let last = await frame();
  let running = true;
  const tick = (t: number) => {
    dt.push(t - last);
    last = t;
    if (running) requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
  for (const [center, zoom] of legs) {
    m.easeTo({ center, zoom, duration: 2000 });
    await sleep(2100);
  }
  running = false;
  const ms = dt.slice(3);
  const mean = ms.reduce((a, b) => a + b, 0) / ms.length;
  return { frames: ms.length, mean_fps: +(1000 / mean).toFixed(1), ...summary(ms), over_33ms: ms.filter((x) => x > 33.4).length };
}

type Method = "strings" | "gpu";

async function recolour(m: MlMap, method: Method, provinces: number) {
  const paint =
    method === "strings"
      ? { "fill-color": ["coalesce", ["feature-state", "c"], "#9a9a96"] }
      : { "fill-color": ["interpolate", ["linear"], ["coalesce", ["feature-state", "v"], 0], ...BREAKS.flatMap((b, k) => [b, RAMP[k + 1]])] };
  m.addLayer({ id: "bench-fill", type: "fill", source: "map", "source-layer": "counties", minzoom: 4, paint } as never, "county-lines");
  m.jumpTo({ center: [10, 50], zoom: 4.5 });
  await settle(m);
  const ipc: number[] = [], apply: number[] = [], screen: number[] = [], worstFrame: number[] = [];
  let counties = 0;
  for (let k = 0; k < UPDATES; k++) {
    const source = (k * 997) % provinces;
    const t0 = performance.now();
    const v = new Float32Array(await invoke<ArrayBuffer>("county_costs", { source }));
    const t1 = performance.now();
    counties = v.length;
    for (let i = 0; i < v.length; i++) {
      const id = { source: "map", sourceLayer: "counties", id: i + 1 };
      if (method === "gpu") m.setFeatureState(id, { v: v[i] });
      else {
        let c = 0;
        while (c < BREAKS.length && v[i] >= BREAKS[c]) c++;
        m.setFeatureState(id, { c: RAMP[c] });
      }
    }
    const t2 = performance.now();
    const f1 = await frame();
    const f2 = await frame();
    const f3 = await frame();
    ipc.push(t1 - t0);
    apply.push(t2 - t1);
    screen.push(f2 - t0);
    worstFrame.push(Math.max(f1 - t2, f2 - f1, f3 - f2));
    await sleep(100);
  }
  const rendered = m.queryRenderedFeatures(undefined, { layers: ["bench-fill"] }).length;
  m.removeLayer("bench-fill");
  return { method, counties, rendered, query_and_ipc_ms: summary(ipc), js_apply_ms: summary(apply), to_screen_ms: summary(screen), worst_frame_ms: summary(worstFrame) };
}

export async function runBench(provinces: number) {
  const w = window as unknown as { ceterisMap?: MlMap };
  log("start");
  while (!w.ceterisMap?.loaded()) await sleep(100);
  const m = w.ceterisMap;
  log(`map loaded; visibility ${document.visibilityState}`);
  const f0 = performance.now();
  await Promise.race([frame(), sleep(2000)]);
  log(`first animation frame after ${(performance.now() - f0).toFixed(0)} ms`);
  m.setLayoutProperty("county-lines", "visibility", "visible");
  m.jumpTo({ center: [20, 30], zoom: 1.4 });
  await settle(m);
  const report = {
    webview: navigator.userAgent,
    viewport: [innerWidth, innerHeight, devicePixelRatio],
    flight_cold: await flight(m).finally(() => log("flight 1 done")),
    flight_warm: await flight(m),
    recolour_strings: await recolour(m, "strings", provinces).finally(() => log("strings done")),
    recolour_gpu: await recolour(m, "gpu", provinces),
  };
  await invoke("bench_report", { report: JSON.stringify(report) });
}
