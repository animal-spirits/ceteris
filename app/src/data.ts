import { convertFileSrc } from "@tauri-apps/api/core";

export const inTauri = "__TAURI_INTERNALS__" in window;

/** A file in the pack's web directory: served by pack:// in the desktop app (ADR 0010), by Vite in a browser. */
export const packUrl = (name: string) => (inTauri ? convertFileSrc(`web/${name}`, "pack") : `${location.origin}/${name}`);

// Province attributes from the pack's web/provinces.json (columnar; index = province index).

export const RESOURCE_GROUPS = ["precious", "base", "ferrous", "energy", "industrial"] as const;
export type ResourceGroup = (typeof RESOURCE_GROUPS)[number];

export interface Countries {
  code: string[];
  name: string[];
  sovereign: string[];
  colour: number[];
}

export interface Provinces {
  id: string[];
  name: string[];
  country: number[];
  area_km2: number[];
  elev_mean_m: number[];
  tri_m: number[];
  terrain: number[];
  coastal: boolean[];
  dev_slice: boolean[];
  ports: number[];
  airports: number[];
  rail: boolean[];
  res_precious: number[];
  res_base: number[];
  res_ferrous: number[];
  res_energy: number[];
  res_industrial: number[];
  countries: Countries;
}

export const TERRAIN = ["Plains", "Plateau", "Hills", "Mountains"];

export interface Counties {
  name: string[];
  level: string[];
  province: number[];
  area_km2: number[];
  elev_mean_m: number[];
  tri_m: number[];
}

export async function loadCounties(): Promise<Counties> {
  const r = await fetch(packUrl("counties.json"));
  if (!r.ok) throw new Error("counties not found: rebuild the pack");
  return r.json();
}

export async function loadProvinces(): Promise<Provinces> {
  const r = await fetch(packUrl("provinces.json"));
  if (!r.ok) throw new Error("pack not found: build it with `cargo run --release --features ceteris-cli/geos --bin ceteris -- pack build`");
  return r.json();
}

export function resources(p: Provinces, group: ResourceGroup | "all", i: number): number {
  if (group !== "all") return p[`res_${group}`][i];
  return RESOURCE_GROUPS.reduce((s, g) => s + p[`res_${g}`][i], 0);
}
