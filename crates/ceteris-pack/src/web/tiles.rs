//! The map's vector tiles: one PMTiles archive with a layer per geometry set, made by
//! tippecanoe (ADR 0009), which must be on PATH.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use geo::Geometry;
use sha2::{Digest, Sha256};

use super::{MAX_ZOOM, geojson};

/// One layer of the archive: its name, geometries and the zoom from which they are drawn.
pub struct Layer<'a> {
    pub name: &'a str,
    pub geoms: &'a [Geometry],
    pub minzoom: u32,
}

/// tippecanoe's arguments for an archive at `path` of the GeoJSON files `layers` (name, path),
/// in order.
pub fn argv(path: &Path, layers: &[(&str, &Path)]) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-q", "-f", "-o"].map(OsString::from).into();
    args.push(path.into());
    args.extend(
        [
            "-Z0".to_owned(),
            format!("-z{MAX_ZOOM}"),
            "--no-feature-limit".to_owned(),
            "--no-tile-size-limit".to_owned(),
            "--detect-shared-borders".to_owned(),
        ]
        .map(OsString::from),
    );
    for (name, src) in layers {
        let mut layer = OsString::from(format!("{name}:"));
        layer.push(src);
        args.extend([OsString::from("-L"), layer]);
    }
    args
}

pub(crate) fn hex(digest: impl AsRef<[u8]>) -> String {
    digest.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// Writes the archive and returns `web/map.pmtiles:<layer>` -> SHA-256 of the layer's GeoJSON.
/// tippecanoe writes the same tiles in a varying order, so the inputs stand for the archive.
pub fn write(path: &Path, layers: &[Layer]) -> Result<BTreeMap<String, String>, String> {
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut digests = BTreeMap::new();
    let mut sources = Vec::with_capacity(layers.len());
    for layer in layers {
        let src = tmp.path().join(format!("{}.geojson", layer.name));
        let text = geojson::text(layer.geoms, layer.minzoom)
            .map_err(|e| format!("layer {}: {e}", layer.name))?;
        std::fs::write(&src, &text).map_err(|e| format!("{}: {e}", src.display()))?;
        digests.insert(
            format!("web/map.pmtiles:{}", layer.name),
            hex(Sha256::digest(text)),
        );
        sources.push((layer.name, src));
    }
    let named: Vec<(&str, &Path)> = sources.iter().map(|(n, p)| (*n, p.as_path())).collect();
    let status = Command::new("tippecanoe")
        .args(argv(path, &named))
        .status()
        .map_err(|e| format!("tippecanoe: {e}"))?;
    if !status.success() {
        return Err(format!("tippecanoe: {status}"));
    }
    Ok(digests)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_is_build_py_s() {
        let args = argv(
            Path::new("/p/web/map.pmtiles"),
            &[
                ("provinces", Path::new("/t/provinces.geojson")),
                ("counties", Path::new("/t/counties.geojson")),
            ],
        );
        let want = [
            "-q",
            "-f",
            "-o",
            "/p/web/map.pmtiles",
            "-Z0",
            "-z9",
            "--no-feature-limit",
            "--no-tile-size-limit",
            "--detect-shared-borders",
            "-L",
            "provinces:/t/provinces.geojson",
            "-L",
            "counties:/t/counties.geojson",
        ];
        assert_eq!(args, want.map(OsString::from));
    }
}
