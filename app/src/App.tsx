import { useEffect, useMemo, useState } from "react";
import { MapView } from "./MapView";
import { Inspector } from "./Inspector";
import { type Provinces, RESOURCE_GROUPS, type ResourceGroup, loadProvinces } from "./data";
import { type CostTree, costFrom } from "./engine";
import { type MapMode, paint } from "./modes";
import { Fps } from "./Fps";

const MODES: [MapMode, string][] = [
  ["political", "Political"],
  ["terrain", "Terrain"],
  ["resources", "Resources"],
  ["cost", "Freight cost"],
];

export function App() {
  const [p, setP] = useState<Provinces | null>(null);
  const [mode, setMode] = useState<MapMode>("political");
  const [group, setGroup] = useState<ResourceGroup | "all">("all");
  const [selected, setSelected] = useState<number | null>(null);
  const [origin, setOrigin] = useState<number | null>(null);
  const [tree, setTree] = useState<CostTree | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadProvinces().then(setP, (e) => setError(String(e)));
  }, []);

  useEffect(() => {
    if (!p || origin === null) return;
    costFrom(origin, p.id.length).then(setTree, (e) => setError(String(e)));
  }, [p, origin]);

  const painted = useMemo(() => (p ? paint(mode, p, group, tree) : null), [p, mode, group, tree]);

  const onSelect = (i: number) => {
    setSelected(i);
    if (mode === "cost") setOrigin(i);
  };

  return (
    <div className="app">
      <MapView fill={painted?.fill ?? null} selected={selected} origin={mode === "cost" ? origin : null} onSelect={onSelect} />
      <nav className="panel modes" aria-label="Map mode">
        {MODES.map(([m, label]) => (
          <button key={m} className={m === mode ? "active" : ""} onClick={() => setMode(m)}>
            {label}
          </button>
        ))}
        {mode === "resources" && (
          <select value={group} onChange={(e) => setGroup(e.target.value as ResourceGroup | "all")} aria-label="Resource group">
            <option value="all">all groups</option>
            {RESOURCE_GROUPS.map((g) => (
              <option key={g} value={g}>
                {g}
              </option>
            ))}
          </select>
        )}
      </nav>
      {painted && (
        <section className="panel legend" aria-label="Legend">
          <h3>{painted.title}</h3>
          <ul>
            {painted.legend.map((item) => (
              <li key={item.label}>
                <span className="swatch" style={{ background: item.colour }} />
                {item.label}
              </li>
            ))}
          </ul>
          {painted.note && <p>{painted.note}</p>}
        </section>
      )}
      {p && selected !== null && <Inspector p={p} i={selected} origin={origin} tree={tree} onClose={() => setSelected(null)} />}
      {error && (
        <div className="panel error" role="alert">
          {error}
        </div>
      )}
      <Fps />
    </div>
  );
}
