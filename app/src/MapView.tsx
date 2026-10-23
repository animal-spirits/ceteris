import { useEffect, useRef } from "react";
import { Map as MlMap, NavigationControl } from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { OCEAN } from "./modes";

interface Props {
  fill: string[] | null;
  selected: number | null;
  origin: number | null;
  onSelect: (province: number) => void;
}

const SOURCE = "provinces";

/** WebGL map of provinces coloured through feature state, so a mode change never re-uploads geometry. */
export function MapView({ fill, selected, origin, onSelect }: Props) {
  const container = useRef<HTMLDivElement>(null);
  const map = useRef<MlMap | null>(null);
  const pending = useRef<(() => void)[] | null>([]);
  const marks = useRef<{ selected: number | null; origin: number | null }>({ selected: null, origin: null });
  const select = useRef(onSelect);
  select.current = onSelect;

  useEffect(() => {
    const m = new MlMap({
      container: container.current!,
      style: {
        version: 8,
        sources: {
          [SOURCE]: { type: "geojson", data: "/provinces.geojson" },
          countries: { type: "geojson", data: "/countries.geojson" },
        },
        layers: [
          { id: "ocean", type: "background", paint: { "background-color": OCEAN } },
          {
            id: "fill",
            type: "fill",
            source: SOURCE,
            paint: {
              "fill-color": ["coalesce", ["feature-state", "fill"], "#9a9a96"],
              "fill-opacity": ["case", ["boolean", ["feature-state", "hover"], false], 0.8, 1],
            },
          },
          {
            id: "province-lines",
            type: "line",
            source: SOURCE,
            minzoom: 3,
            paint: { "line-color": "#1c1c1a", "line-opacity": 0.25, "line-width": 0.5 },
          },
          {
            id: "country-lines",
            type: "line",
            source: "countries",
            paint: { "line-color": "#1c1c1a", "line-width": ["interpolate", ["linear"], ["zoom"], 1, 0.6, 6, 1.6] },
          },
          {
            id: "marked",
            type: "line",
            source: SOURCE,
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
      maxZoom: 9,
      attributionControl: { compact: true, customAttribution: "Natural Earth; NOAA ETOPO 2022; OurAirports; USGS MRDS" },
    });
    m.addControl(new NavigationControl({ showCompass: false }), "bottom-right");
    let hover: number | null = null;
    m.on("mousemove", "fill", (e) => {
      const id = e.features?.[0]?.id as number | undefined;
      if (id === undefined || id === hover) return;
      if (hover !== null) m.setFeatureState({ source: SOURCE, id: hover }, { hover: false });
      hover = id;
      m.setFeatureState({ source: SOURCE, id }, { hover: true });
      m.getCanvas().style.cursor = "pointer";
    });
    m.on("mouseleave", "fill", () => {
      if (hover !== null) m.setFeatureState({ source: SOURCE, id: hover }, { hover: false });
      hover = null;
      m.getCanvas().style.cursor = "";
    });
    m.on("click", "fill", (e) => {
      const id = e.features?.[0]?.id;
      if (typeof id === "number") select.current(id);
    });
    m.on("load", () => {
      const queued = pending.current ?? [];
      pending.current = null;
      queued.forEach((f) => f());
    });
    map.current = m;
    (window as unknown as { ceterisMap: MlMap }).ceterisMap = m;
    return () => m.remove();
  }, []);

  useEffect(() => {
    const m = map.current;
    if (!m || !fill) return;
    const apply = () => {
      for (let i = 0; i < fill.length; i++) m.setFeatureState({ source: SOURCE, id: i }, { fill: fill[i] });
    };
    if (pending.current) pending.current.push(apply);
    else apply();
  }, [fill]);

  useEffect(() => {
    const m = map.current;
    if (!m) return;
    const apply = () => {
      const prev = marks.current;
      if (prev.selected !== null) m.setFeatureState({ source: SOURCE, id: prev.selected }, { selected: false });
      if (prev.origin !== null) m.setFeatureState({ source: SOURCE, id: prev.origin }, { origin: false });
      if (selected !== null) m.setFeatureState({ source: SOURCE, id: selected }, { selected: true });
      if (origin !== null) m.setFeatureState({ source: SOURCE, id: origin }, { origin: true });
      marks.current = { selected, origin };
    };
    if (pending.current) pending.current.push(apply);
    else apply();
  }, [selected, origin]);

  return <div ref={container} className="map" />;
}
