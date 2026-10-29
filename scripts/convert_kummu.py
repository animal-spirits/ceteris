# /// script
# requires-python = ">=3.12"
# dependencies = ["numpy==2.5.4", "rasterio==1.5.2"]
# ///
"""Extracts the year-2000 band of the Kummu et al. gridded GDP NetCDF-4 file into a GeoTIFF.

The pack reads GeoTIFFs only; the NetCDF stays pinned in crates/ceteris-pack/config/fetch.toml
as the provenance source. An owner-side, one-off conversion: the output is pinned by sha256, so
it runs only to reproduce that file, in the environment declared above (the rasterio wheel
bundles GDAL with NetCDF support):

    uv run scripts/convert_kummu.py
"""

import hashlib
from pathlib import Path

import rasterio

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "data" / "cache" / "GDP_PPP_1990_2015_5arcmin_v2.nc"
TARGET = ROOT / "data" / "cache" / "kummu_gdp_ppp_2000_5arcmin.tif"
YEAR = "2000"


def main() -> None:
    with rasterio.open(SOURCE) as src:
        band = next(b for b in range(1, src.count + 1) if src.tags(b).get("NETCDF_DIM_time") == YEAR)
        profile = {
            "driver": "GTiff",
            "dtype": "float32",
            "count": 1,
            "width": src.width,
            "height": src.height,
            "transform": src.transform,
            "crs": src.crs,
            "nodata": -9.0,
            "compress": "deflate",
            "tiled": True,
            "blockxsize": 256,
            "blockysize": 256,
        }
        data = src.read(band).astype("float32")
    with rasterio.open(TARGET, "w", **profile) as dst:
        dst.write(data, 1)
    digest = hashlib.sha256(TARGET.read_bytes()).hexdigest()
    print(f"sha256 = {digest}\nbytes = {TARGET.stat().st_size}")


if __name__ == "__main__":
    main()
