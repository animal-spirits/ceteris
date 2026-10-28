//! Helpers of the reader tests: fixture files and the fetch cache.

#![allow(dead_code)]

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use ceteris_pack::fetch;
use zip::write::SimpleFileOptions;

/// A cache file, or None (with a note) when it has not been fetched: data tests skip without it.
pub fn cached(name: &str) -> Option<PathBuf> {
    let path = fetch::path_of(name);
    if path.exists() {
        Some(path)
    } else {
        eprintln!(
            "skipping: no {name} in {}; fetch it with `ceteris pack fetch`",
            path.parent().unwrap().display()
        );
        None
    }
}

/// Writes a zip of `(member, text)` entries.
pub fn write_zip(path: &Path, members: &[(&str, &str)]) -> PathBuf {
    let mut z = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, text) in members {
        z.start_file(*name, SimpleFileOptions::default()).unwrap();
        z.write_all(text.as_bytes()).unwrap();
    }
    z.finish().unwrap();
    path.to_owned()
}

const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// Writes an xlsx with the given shared strings and sheets (name, `<sheetData>` content); style
/// 1 is a date format.
pub fn write_xlsx(path: &Path, strings: &[&str], sheets: &[(&str, &str)]) -> PathBuf {
    let book: String = sheets
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            format!(
                r#"<sheet name="{name}" sheetId="{}" r:id="rId{}"/>"#,
                i + 1,
                i + 1
            )
        })
        .collect();
    let rels: String = (1..=sheets.len())
        .map(|i| format!(r#"<Relationship Id="rId{i}" Type="{REL}/worksheet" Target="worksheets/sheet{i}.xml"/>"#))
        .collect();
    let sst: String = strings
        .iter()
        .map(|s| format!("<si><t>{s}</t></si>"))
        .collect();
    let mut members = vec![
        (
            "_rels/.rels".to_owned(),
            format!(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/officeDocument" Target="xl/workbook.xml"/></Relationships>"#
            ),
        ),
        (
            "xl/workbook.xml".to_owned(),
            format!(
                r#"<workbook xmlns="{MAIN}" xmlns:r="{REL}"><sheets>{book}</sheets></workbook>"#
            ),
        ),
        (
            "xl/_rels/workbook.xml.rels".to_owned(),
            format!(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#
            ),
        ),
        (
            "xl/sharedStrings.xml".to_owned(),
            format!(r#"<sst xmlns="{MAIN}">{sst}</sst>"#),
        ),
        (
            "xl/styles.xml".to_owned(),
            format!(
                r#"<styleSheet xmlns="{MAIN}"><cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="14" applyNumberFormat="1"/></cellXfs></styleSheet>"#
            ),
        ),
    ];
    for (i, (_, data)) in sheets.iter().enumerate() {
        members.push((
            format!("xl/worksheets/sheet{}.xml", i + 1),
            format!(r#"<worksheet xmlns="{MAIN}"><sheetData>{data}</sheetData></worksheet>"#),
        ));
    }
    let refs: Vec<(&str, &str)> = members
        .iter()
        .map(|(n, t)| (n.as_str(), t.as_str()))
        .collect();
    write_zip(path, &refs)
}

/// Deterministic uniform numbers in [lo, hi) for hand-made tables.
pub struct Uniform(u64);

impl Uniform {
    pub fn new(seed: u64) -> Self {
        Uniform(
            seed.wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407),
        )
    }

    pub fn next(&mut self, lo: f64, hi: f64) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        lo + (hi - lo) * ((self.0 >> 11) as f64 / (1u64 << 53) as f64)
    }
}

pub fn approx(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs()) || a == b
}
