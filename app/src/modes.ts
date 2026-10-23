// Map modes (Section 20.1): one fill colour per province, plus a legend.
// Palettes validated with the dataviz validator against the ocean surface.

import { type Provinces, type ResourceGroup, TERRAIN, resources } from "./data";
import type { CostTree } from "./engine";

export type MapMode = "political" | "terrain" | "resources" | "cost";

export const OCEAN = "#4a6680";
export const NO_DATA = "#9a9a96";
// Categorical slots in fixed order; country borders are the secondary encoding.
const COUNTRY = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#6250d6", "#e34948"];
const TERRAIN_RAMP = ["#e3d3b0", "#c6a571", "#9a7342", "#5e4424"];
const SEQUENTIAL = ["#fde0c5", "#f9b98a", "#f28f53", "#e0662b", "#b84a1b", "#8a3512", "#5c230b"];

export interface LegendItem {
  colour: string;
  label: string;
}

export interface Painted {
  fill: string[];
  legend: LegendItem[];
  title: string;
  note?: string;
}

/** Class breaks at quantiles of the positive finite values, so each colour covers a similar share of provinces. */
function quantileBreaks(values: number[], classes: number): number[] {
  const v = values.filter((x) => Number.isFinite(x) && x > 0).sort((a, b) => a - b);
  if (v.length === 0) return [];
  const breaks: number[] = [];
  for (let k = 1; k < classes; k++) breaks.push(v[Math.min(v.length - 1, Math.floor((k * v.length) / classes))]);
  // Ties (common with small counts) would leave classes empty.
  return [...new Set(breaks)].filter((b) => b > v[0]);
}

function classify(x: number, breaks: number[]): number {
  let k = 0;
  while (k < breaks.length && x >= breaks[k]) k++;
  return k;
}

const fmt = (x: number) => (x >= 100 || Number.isInteger(x) ? Math.round(x).toLocaleString("en") : x.toPrecision(2));

/** Classes are [lower, next lower); counts label their integer ranges, e.g. "4 to 6". */
function sequential(values: number[], unit: string, zeroLabel?: string): { fill: string[]; legend: LegendItem[] } {
  const breaks = quantileBreaks(values, SEQUENTIAL.length);
  const counts = values.every((x) => Number.isInteger(x));
  const fill = values.map((x) =>
    !Number.isFinite(x) || (x <= 0 && zeroLabel) ? NO_DATA : SEQUENTIAL[classify(x, breaks)],
  );
  const lowest = zeroLabel ? Math.min(...values.filter((x) => x > 0)) : 0;
  const legend = [lowest, ...breaks].map((lo, k) => {
    const hi = k < breaks.length ? (counts ? breaks[k] - 1 : breaks[k]) : null;
    const range = hi === null ? `${fmt(lo)}+` : hi === lo ? fmt(lo) : `${fmt(lo)} to ${fmt(hi)}`;
    return { colour: SEQUENTIAL[k], label: `${range} ${unit}` };
  });
  if (zeroLabel) legend.unshift({ colour: NO_DATA, label: zeroLabel });
  return { fill, legend };
}

export function paint(mode: MapMode, p: Provinces, group: ResourceGroup | "all", tree: CostTree | null): Painted {
  const n = p.id.length;
  switch (mode) {
    case "political":
      return {
        title: "Countries, 1 January 2000",
        fill: p.country.map((c) => COUNTRY[p.countries.colour[c]]),
        legend: [],
        note: "Colours only separate neighbours; click a province for its country.",
      };
    case "terrain":
      return {
        title: "Terrain",
        fill: p.terrain.map((t) => TERRAIN_RAMP[t]),
        legend: TERRAIN.map((label, k) => ({ colour: TERRAIN_RAMP[k], label })),
        note: "From ETOPO 2022 elevation and ruggedness (TRI).",
      };
    case "resources": {
      const values = Array.from({ length: n }, (_, i) => resources(p, group, i));
      return {
        title: `Mineral sites, ${group}`,
        ...sequential(values, "sites", "none recorded"),
        note: "USGS MRDS producing and past-producing sites; about 89% are in the United States.",
      };
    }
    case "cost":
      if (!tree) return { title: "Freight cost per tonne", fill: Array(n).fill(NO_DATA), legend: [], note: "Click a province to set the origin." };
      return {
        title: "Freight cost per tonne from origin",
        ...sequential(Array.from(tree.cost), "USD/t"),
        note: "Least-cost path over road, rail, sea and air; 2000 USD, prior parameters (P04).",
      };
  }
}
