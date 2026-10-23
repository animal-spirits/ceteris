import { type Counties, type Provinces, RESOURCE_GROUPS, TERRAIN } from "./data";
import type { CostTree } from "./engine";

interface Props {
  p: Provinces;
  i: number;
  counties: Counties | null;
  county: number | null;
  origin: number | null;
  tree: CostTree | null;
  onClose: () => void;
}

const num = (x: number, digits = 0) => x.toLocaleString("en", { maximumFractionDigits: digits });

/** Every attribute of one province (Section 20.2, province inspector). */
export function Inspector({ p, i, counties, county, origin, tree, onClose }: Props) {
  const c = p.country[i];
  const rows: [string, string][] = [
    ["Province ID", p.id[i]],
    ["Country (2000)", `${p.countries.name[c]} (${p.countries.code[c]})`],
    ["Sovereign", p.countries.sovereign[c]],
    ["Area", `${num(p.area_km2[i])} km²`],
    ["Mean elevation", `${num(p.elev_mean_m[i])} m`],
    ["Ruggedness (TRI)", `${num(p.tri_m[i], 1)} m`],
    ["Terrain", TERRAIN[p.terrain[i]]],
    ["Coastal", p.coastal[i] ? "yes" : "no"],
    ["Ports", String(p.ports[i])],
    ["Airports", String(p.airports[i])],
    ["Rail border crossing", p.rail[i] ? "yes" : "no"],
    ["Development slice", p.dev_slice[i] ? "yes" : "no"],
    ...RESOURCE_GROUPS.map((g): [string, string] => [`Sites, ${g}`, String(p[`res_${g}`][i])]),
  ];
  // A click near a border can hit a county clipped to the neighbouring province; show it only if it nests here.
  if (counties && county !== null && counties.province[county] === i) {
    rows.push(
      ["County", counties.name[county] || "unnamed"],
      ["County level", counties.level[county]],
      ["County area", `${num(counties.area_km2[county])} km²`],
      ["County elevation", `${num(counties.elev_mean_m[county])} m`],
      ["County TRI", `${num(counties.tri_m[county], 1)} m`],
    );
  }
  if (tree && origin !== null) {
    rows.push(
      ["Origin", p.name[origin]],
      ["Freight cost", `${num(tree.cost[i], 2)} USD/t`],
      ["Transit time", `${num(tree.hours[i] / 24, 1)} days`],
    );
  }
  return (
    <aside className="panel inspector">
      <header>
        <h2>{p.name[i]}</h2>
        <button onClick={onClose} aria-label="Close inspector">×</button>
      </header>
      <dl>
        {rows.map(([k, v]) => (
          <div key={k}>
            <dt>{k}</dt>
            <dd>{v}</dd>
          </div>
        ))}
      </dl>
    </aside>
  );
}
