"""H3 base raster: cell membership and terrain statistics (Section 5.1)."""

from pathlib import Path

import h3
import numpy as np
import rasterio

from .provinces import Province, cells_of

# Samples per cell are its children this many resolutions finer (7^2 = 49).
SAMPLE_DEPTH = 2
FINEST = 9


def assign_cells(provinces: list[Province], res: int) -> tuple[list[str], np.ndarray]:
    """Cells and their province index. A cell whose centre lies in two provinces goes to the lower index.

    Provinces too small to own a cell centre at `res` get cells at the first finer resolution that has one.
    """
    owner: dict[str, int] = {}
    for i, p in enumerate(provinces):
        r = res
        cells = cells_of(p.geom, r)
        while not cells and r < FINEST:
            r += 1
            cells = cells_of(p.geom, r)
        if not cells:
            cells = {h3.latlng_to_cell(p.lat, p.lon, FINEST)}
        for c in cells:
            owner.setdefault(c, i)
    ids = sorted(owner, key=lambda c: (owner[c], c))
    return ids, np.array([owner[c] for c in ids], dtype=np.uint32)


def terrain(cells: list[str], etopo: Path) -> dict[str, np.ndarray]:
    """Per cell: mean and standard deviation of elevation, and mean terrain ruggedness index (Riley et al. 1999)."""
    samples, of_cell = [], []
    for i, c in enumerate(cells):
        kids = h3.cell_to_children(c, h3.get_resolution(c) + SAMPLE_DEPTH)
        samples.extend(h3.cell_to_latlng(k) for k in sorted(kids))
        of_cell.extend([i] * len(kids))
    lat, lon = np.array(samples).T
    of_cell = np.array(of_cell)
    with rasterio.open(etopo) as ds:
        z = ds.read(1).astype(np.float64)
        rows, cols = rasterio.transform.rowcol(ds.transform, lon, lat)
    h, w = z.shape
    rows = np.clip(np.asarray(rows), 0, h - 1)
    cols = np.asarray(cols) % w
    centre = z[rows, cols]
    tri = np.zeros_like(centre)
    for dr in (-1, 0, 1):
        for dc in (-1, 0, 1):
            if dr or dc:
                tri += np.abs(z[np.clip(rows + dr, 0, h - 1), (cols + dc) % w] - centre)
    tri /= 8
    n = np.bincount(of_cell, minlength=len(cells))
    mean = np.bincount(of_cell, centre, len(cells)) / n
    var = np.bincount(of_cell, centre**2, len(cells)) / n - mean**2
    return {
        "elev_mean_m": mean,
        "elev_std_m": np.sqrt(np.maximum(var, 0.0)),
        "tri_m": np.bincount(of_cell, tri, len(cells)) / n,
    }


def cell_area_km2(cells: list[str]) -> np.ndarray:
    return np.array([h3.cell_area(c, unit="km^2") for c in cells])


def province_means(owner: np.ndarray, weight: np.ndarray, values: np.ndarray, n: int) -> np.ndarray:
    return np.bincount(owner, weight * values, n) / np.bincount(owner, weight, n)
