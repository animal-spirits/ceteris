// Least-cost queries to the Rust core: Tauri IPC in the desktop app, the development
// engine over HTTP in a plain browser. Both return little-endian f32 cost then hours per province.

import { invoke } from "@tauri-apps/api/core";

export interface CostTree {
  cost: Float32Array;
  hours: Float32Array;
}

const inTauri = "__TAURI_INTERNALS__" in window;

async function bytes(source: number): Promise<ArrayBuffer> {
  if (inTauri) return invoke<ArrayBuffer>("cost_from", { source });
  const r = await fetch(`/engine/cost/${source}`);
  if (!r.ok) throw new Error(`engine: ${r.status}; start it with \`cargo run --release -p ceteris-geo --example engine\``);
  return r.arrayBuffer();
}

export async function costFrom(source: number, provinces: number): Promise<CostTree> {
  const buf = await bytes(source);
  const all = new Float32Array(buf);
  if (all.length !== 2 * provinces) throw new Error(`engine returned ${all.length} values for ${provinces} provinces`);
  return { cost: all.subarray(0, provinces), hours: all.subarray(provinces) };
}
