import { useEffect, useRef } from "react";
import { Map as MlMap, NavigationControl, addProtocol, type FeatureIdentifier } from "maplibre-gl";
import { Protocol } from "pmtiles";
import "maplibre-gl/dist/maplibre-gl.css";
import { OCEAN } from "./modes";

interface Props {
  fill: string[] | null;
  selected: number | null;
  origin: number | null;
  counties: boolean;
  onSelect: (province: number, county: number | null) => void;
}

addProtocol("pmtiles", new Protocol().tile);

const SOURCE = "map";
// Tile features are numbered from 1 (ADR 0009); provinces and counties are indexed from 0.
const province = (i: number): FeatureIdentifier => ({ source: SOURCE, sourceLayer: "provinces", id: i + 1 });
const COUNTY_MINZOOM = 4;

/** Vector-tiled map of provinces coloured through feature state, so a mode change never re-uploads geometry. */
export function MapView({ fill, selected, origin, counties, onSelect }: Props) {
  const container = useRef<HTMLDivElement>(null);
  const map = useRef<MlMap | null>(null);
  // Updates wait here until the style loads; they run against whichever map is live then.
  const pending = useRef<((m: MlMap) => void)[] | null>([]);
  const marks = useRef<{ selected: number | null; origin: number | null }>({ selected: null, origin: null });
  const select = useRef(onSelect);
  select.current = onSelect;

  useEffect(() => {
    pending.current = [];
    const m = new MlMap({
      container: container.current!,
      style: {
        version: 8,
        sources: { [SOURCE]: { type: "vector", url: `pmtiles://${location.origin}/map.pmtiles` } },
        layers: [
          { id: "ocean", type: "background", paint: { "background-color": OCEAN } },
          {
            id: "fill",
            type: "fill",
            source: SOURCE,
            "source-layer": "provinces",
            paint: {
              "fill-color": ["coalesce", ["feature-state", "fill"], "#9a9a96"],
              "fill-opacity": ["case", ["boolean", ["feature-state", "hover"], false], 0.8, 1],
            },
          },
          {
            // Invisible, so clicks resolve the county whether or not its borders are shown.
            id: "county-hit",
            type: "fill",
            source: SOURCE,
            "source-layer": "counties",
            minzoom: COUNTY_MINZOOM,
            paint: { "fill-opacity": 0 },
          },
          {
            id: "county-lines",
            type: "line",
            source: SOURCE,
            "source-layer": "counties",
            minzoom: COUNTY_MINZOOM,
            layout: { visibility: "none" },
            paint: { "line-color": "#1c1c1a", "line-opacity": 0.3, "line-width": 0.4 },
          },
          {
            id: "province-lines",
            type: "line",
            source: SOURCE,
            "source-layer": "provinces",
            minzoom: 3,
            paint: { "line-color": "#1c1c1a", "line-opacity": 0.35, "line-width": 0.6 },
          },
          {
            id: "country-lines",
            type: "line",
            source: SOURCE,
            "source-layer": "countries",
            paint: { "line-color": "#1c1c1a", "line-width": ["interpolate", ["linear"], ["zoom"], 1, 0.6, 6, 1.6] },
          },
          {
            id: "marked",
            type: "line",
            source: SOURCE,
            "source-layer": "provinces",
            paint: {
              "line-color": ["case", ["boolean", ["feature-state", "origin"], false], "#ffffff", "#0b0b0b"],
              "line-width": 2.5,
              "line-opacity": ["case", ["any", ["boolean", ["feature-state", "selected"], false], ["boolean", ["feature-state", "origin"], false]], 1, 0],
            },
          },
        ],
      },
      center: [15, 50],
      zoom: 3.2,
      minZoom: 1,
      maxZoom: 11,
      attributionControl: { compact: true, customAttribution: "Natural Earth; geoBoundaries; NOAA ETOPO 2022; OurAirports; USGS MRDS" },
    });
    m.addControl(new NavigationControl({ showCompass: false }), "bottom-right");
    let hover: number | null = null;
    m.on("mousemove", "fill", (e) => {
      const id = e.features?.[0]?.id as number | undefined;
      if (id === undefined || id - 1 === hover) return;
      if (hover !== null) m.setFeatureState(province(hover), { hover: false });
      hover = id - 1;
      m.setFeatureState(province(hover), { hover: true });
      m.getCanvas().style.cursor = "pointer";
    });
    m.on("mouseleave", "fill", () => {
      if (hover !== null) m.setFeatureState(province(hover), { hover: false });
      hover = null;
      m.getCanvas().style.cursor = "";
    });
    m.on("click", "fill", (e) => {
      const id = e.features?.[0]?.id;
      if (typeof id !== "number") return;
      const county = m.queryRenderedFeatures(e.point, { layers: ["county-hit"] })[0]?.id;
      select.current(id - 1, typeof county === "number" ? county - 1 : null);
    });
    m.on("load", () => {
      const queued = pending.current ?? [];
      pending.current = null;
      queued.forEach((f) => f(m));
    });
    map.current = m;
    (window as unknown as { ceterisMap: MlMap }).ceterisMap = m;
    return () => m.remove();
  }, []);

  const whenLoaded = (f: (m: MlMap) => void) => {
    if (pending.current) pending.current.push(f);
    else if (map.current) f(map.current);
  };

  useEffect(() => {
    if (!fill) return;
    whenLoaded((m) => {
      for (let i = 0; i < fill.length; i++) m.setFeatureState(province(i), { fill: fill[i] });
    });
  }, [fill]);

  useEffect(() => {
    whenLoaded((m) => m.setLayoutProperty("county-lines", "visibility", counties ? "visible" : "none"));
  }, [counties]);

  useEffect(() => {
    whenLoaded((m) => {
      const prev = marks.current;
      if (prev.selected !== null) m.setFeatureState(province(prev.selected), { selected: false });
      if (prev.origin !== null) m.setFeatureState(province(prev.origin), { origin: false });
      if (selected !== null) m.setFeatureState(province(selected), { selected: true });
      if (origin !== null) m.setFeatureState(province(origin), { origin: true });
      marks.current = { selected, origin };
    });
  }, [selected, origin]);

  return <div ref={container} className="map" />;
}

