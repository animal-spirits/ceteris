import { useEffect, useRef } from "react";
import { Map as MlMap, NavigationControl, addProtocol, type ExpressionSpecification, type FeatureIdentifier } from "maplibre-gl";
import { Protocol } from "pmtiles";
import "maplibre-gl/dist/maplibre-gl.css";
import { packUrl } from "./data";
import { FillLayer, rgba } from "./fills";
import { OCEAN } from "./modes";

interface Props {
  fill: string[] | null;
  selected: number | null;
  origin: number | null;
  counties: boolean;
  globe: boolean;
  onSelect: (province: number, county: number | null) => void;
}

addProtocol("pmtiles", new Protocol().tile);

const SOURCE = "map";
// Tile features are numbered from 1 (ADR 0009); provinces and counties are indexed from 0.
const province = (i: number): FeatureIdentifier => ({ source: SOURCE, sourceLayer: "provinces", id: i + 1 });
const COUNTY_MINZOOM = 4;
const MARKED_OPACITY: ExpressionSpecification = ["case", ["any", ["boolean", ["feature-state", "selected"], false], ["boolean", ["feature-state", "origin"], false]], 1, 0];

/** Vector-tiled borders and labels from MapLibre; province colours from the GPU fill layer (ADR 0010). */
export function MapView({ fill, selected, origin, counties, globe, onSelect }: Props) {
  const container = useRef<HTMLDivElement>(null);
  const map = useRef<MlMap | null>(null);
  const fills = useRef<FillLayer | null>(null);
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
        sources: { [SOURCE]: { type: "vector", url: `pmtiles://${packUrl("map.pmtiles")}` } },
        layers: [
          { id: "ocean", type: "background", paint: { "background-color": OCEAN } },
          {
            // Invisible: provinces are drawn by the fill layer; this one answers clicks and hovers.
            id: "province-hit",
            type: "fill",
            source: SOURCE,
            "source-layer": "provinces",
            paint: { "fill-opacity": 0 },
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
            paint: { "line-color": "#141b22", "line-opacity": 0.3, "line-width": 0.4 },
          },
          {
            id: "province-lines",
            type: "line",
            source: SOURCE,
            "source-layer": "provinces",
            minzoom: 3,
            paint: { "line-color": "#141b22", "line-opacity": 0.35, "line-width": 0.6 },
          },
          {
            id: "country-lines",
            type: "line",
            source: SOURCE,
            "source-layer": "countries",
            paint: { "line-color": "#0a0f14", "line-width": ["interpolate", ["linear"], ["zoom"], 1, 0.6, 6, 1.6] },
          },
          {
            // Dark casing under the mark, so the pale outline also reads on light fills (terrain plains).
            id: "marked-casing",
            type: "line",
            source: SOURCE,
            "source-layer": "provinces",
            paint: {
              "line-color": "#0a0f14",
              "line-width": 5,
              "line-opacity": MARKED_OPACITY,
            },
          },
          {
            id: "marked",
            type: "line",
            source: SOURCE,
            "source-layer": "provinces",
            paint: {
              "line-color": ["case", ["boolean", ["feature-state", "origin"], false], "#8db6e3", "#ffe7a3"],
              "line-width": 2.5,
              "line-opacity": MARKED_OPACITY,
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
    const layer = new FillLayer("province-fill", packUrl("provinces.fill"));
    fills.current = layer;
    m.on("mousemove", "province-hit", (e) => {
      const id = e.features?.[0]?.id;
      if (typeof id === "number") layer.setHover(id - 1);
      m.getCanvas().style.cursor = "pointer";
    });
    m.on("mouseleave", "province-hit", () => {
      layer.setHover(null);
      m.getCanvas().style.cursor = "";
    });
    m.on("click", "province-hit", (e) => {
      const id = e.features?.[0]?.id;
      if (typeof id !== "number") return;
      const county = m.queryRenderedFeatures(e.point, { layers: ["county-hit"] })[0]?.id;
      select.current(id - 1, typeof county === "number" ? county - 1 : null);
    });
    m.on("load", () => {
      m.addLayer(layer, "province-hit");
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
    whenLoaded(() => fills.current?.setColours(rgba(fill)));
  }, [fill]);

  useEffect(() => {
    whenLoaded((m) => m.setProjection({ type: globe ? "globe" : "mercator" }));
  }, [globe]);

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

