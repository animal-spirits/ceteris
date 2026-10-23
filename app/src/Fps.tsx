import { useEffect, useState } from "react";

/** Frame-rate readout, shown with `?fps` in the URL; used to check the 60 fps gate (Section 21.1, P1). */
export function Fps() {
  const enabled = new URLSearchParams(location.search).has("fps");
  const [fps, setFps] = useState(0);
  useEffect(() => {
    if (!enabled) return;
    let frames = 0;
    let start = performance.now();
    let raf = 0;
    const tick = (now: number) => {
      frames++;
      if (now - start >= 1000) {
        setFps(Math.round((frames * 1000) / (now - start)));
        frames = 0;
        start = now;
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [enabled]);
  return enabled ? <div className="fps">{fps} fps</div> : null;
}
