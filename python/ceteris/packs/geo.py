"""Spherical helpers on WGS84 lon/lat degrees, with a spherical Earth of mean radius."""

import math

import numpy as np
import shapely
from shapely.geometry import box
from shapely.geometry.base import BaseGeometry

R_KM = 6371.0088


def haversine_km(lon1, lat1, lon2, lat2):
    lon1, lat1, lon2, lat2 = map(np.radians, (lon1, lat1, lon2, lat2))
    a = np.sin((lat2 - lat1) / 2) ** 2 + np.cos(lat1) * np.cos(lat2) * np.sin((lon2 - lon1) / 2) ** 2
    return 2 * R_KM * np.arcsin(np.sqrt(np.minimum(a, 1.0)))


def _ring_area(coords: np.ndarray) -> float:
    # Chamberlain and Duquette (2007), the spherical-excess approximation used by d3 and turf.
    lon, lat = np.radians(coords[:, 0]), np.radians(coords[:, 1])
    return float(np.sum((lon[1:] - lon[:-1]) * (2 + np.sin(lat[:-1]) + np.sin(lat[1:])))) * R_KM**2 / 2


def area_km2(geom: BaseGeometry) -> float:
    total = 0.0
    for poly in shapely.get_parts(geom):
        total += abs(_ring_area(np.asarray(poly.exterior.coords)))
        total -= sum(abs(_ring_area(np.asarray(r.coords))) for r in poly.interiors)
    return total


def polygonal(geom: BaseGeometry) -> BaseGeometry:
    """Drop the points and lines that intersections leave behind."""
    parts = [p for p in shapely.get_parts(geom) if p.geom_type in ("Polygon", "MultiPolygon")]
    return shapely.union_all(parts) if parts else shapely.Polygon()


def crosses_antimeridian(geom: BaseGeometry) -> bool:
    minx, _, maxx, _ = geom.bounds
    return minx < -170 and maxx > 170


def unwrap(geom: BaseGeometry) -> BaseGeometry:
    """Shift western-hemisphere parts east by 360 degrees so the geometry is contiguous."""
    parts = [shapely.transform(p, lambda xy: xy + [360, 0]) if p.centroid.x < 0 else p for p in shapely.get_parts(geom)]
    return shapely.union_all(parts)


def wrap(geom: BaseGeometry) -> BaseGeometry:
    west = geom.intersection(box(-180, -90, 180, 90))
    east = shapely.transform(geom.intersection(box(180, -90, 540, 90)), lambda xy: xy - [360, 0])
    return polygonal(shapely.union_all([west, east]))


def anchor(geom: BaseGeometry) -> tuple[float, float]:
    """Transport anchor: centroid of the largest part, or a point inside it if the centroid falls outside."""
    largest = max(shapely.get_parts(geom), key=lambda p: p.area)
    c = largest.centroid
    if not largest.contains(c):
        c = largest.point_on_surface()
    return c.x, c.y


def cos_lat(lat: float) -> float:
    return max(math.cos(math.radians(lat)), 0.05)
