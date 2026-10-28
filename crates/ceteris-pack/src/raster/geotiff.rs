//! Single-band Float32 GeoTIFF read by row windows, georeferenced as GDAL reads it: the affine
//! transform from ModelTransformation or from ModelTiepoint and ModelPixelScale, nodata from the
//! GDAL_NODATA tag. Chunks (tiles or strips) are uncompressed, LZW or deflate, without predictor.

use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;

use tiff::TiffError;
use tiff::decoder::{ChunkType, Decoder};
use tiff::tags::{ByteOrder, Tag};

/// GTRasterTypeGeoKey, and its value for rasters whose tiepoints mark pixel centres.
const RASTER_TYPE_KEY: u16 = 1025;
const PIXEL_IS_POINT: u16 = 2;
const IEEE_FLOAT: u16 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Compression {
    None,
    Lzw,
    Deflate,
}

/// An open GeoTIFF; `transform` is `(a, b, c, d, e, f)` in rasterio's order, mapping
/// (col, row) to `(a * col + b * row + c, d * col + e * row + f)`.
pub struct Raster<R: Read + Seek> {
    decoder: Decoder<R>,
    origin: String,
    pub width: usize,
    pub height: usize,
    pub transform: [f64; 6],
    pub nodata: Option<f64>,
    compression: Compression,
    little_endian: bool,
    chunk_width: usize,
    chunk_height: usize,
    tiled: bool,
    offsets: Vec<u64>,
    lengths: Vec<u64>,
}

/// Opens the GeoTIFF at `path`.
pub fn open(path: &Path) -> Result<Raster<BufReader<File>>, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Raster::new(BufReader::new(file), path.display().to_string())
}

/// Opens a GeoTIFF held in memory (a zip member); `origin` names it in errors.
pub fn from_bytes(bytes: Vec<u8>, origin: &str) -> Result<Raster<Cursor<Vec<u8>>>, String> {
    Raster::new(Cursor::new(bytes), origin.to_owned())
}

impl<R: Read + Seek> Raster<R> {
    fn new(reader: R, origin: String) -> Result<Self, String> {
        let fail = |e: TiffError| format!("{origin}: {e}");
        let mut decoder = Decoder::new(reader).map_err(fail)?;
        let (width, height) = decoder.dimensions().map_err(fail)?;
        let number = |d: &mut Decoder<R>, tag: Tag, default: u16| {
            d.find_tag_unsigned::<u16>(tag)
                .map(|v| v.unwrap_or(default))
                .map_err(fail)
        };
        let format = (
            number(&mut decoder, Tag::SamplesPerPixel, 1)?,
            number(&mut decoder, Tag::BitsPerSample, 1)?,
            number(&mut decoder, Tag::SampleFormat, 1)?,
        );
        if format != (1, 32, IEEE_FLOAT) {
            return Err(format!("{origin}: not a single-band Float32 raster"));
        }
        if number(&mut decoder, Tag::Predictor, 1)? != 1 {
            return Err(format!("{origin}: predictors are not supported"));
        }
        let compression = match number(&mut decoder, Tag::Compression, 1)? {
            1 => Compression::None,
            5 => Compression::Lzw,
            8 | 32946 => Compression::Deflate,
            c => return Err(format!("{origin}: compression {c} is not supported")),
        };
        let transform = geotransform(&mut decoder).map_err(|e| format!("{origin}: {e}"))?;
        let nodata = match decoder.find_tag(Tag::GdalNodata).map_err(fail)? {
            None => None,
            Some(v) => {
                let text = v.into_string().map_err(fail)?;
                let text = text.trim_matches(|c: char| c == '\0' || c.is_whitespace());
                Some(
                    text.parse::<f64>()
                        .map_err(|e| format!("{origin}: GDAL_NODATA {text:?}: {e}"))?,
                )
            }
        };
        let tiled = decoder.get_chunk_type() == ChunkType::Tile;
        let (chunk_width, chunk_height) = decoder.chunk_dimensions();
        let (offsets, lengths) = if tiled {
            (Tag::TileOffsets, Tag::TileByteCounts)
        } else {
            (Tag::StripOffsets, Tag::StripByteCounts)
        };
        let mut chunks = |tag: Tag| {
            decoder
                .find_tag_unsigned_vec::<u64>(tag)
                .map_err(fail)?
                .ok_or_else(|| format!("{origin}: no {tag:?}"))
        };
        let (offsets, lengths) = (chunks(offsets)?, chunks(lengths)?);
        let little_endian = decoder.byte_order() == ByteOrder::LittleEndian;
        Ok(Raster {
            decoder,
            origin,
            width: width as usize,
            height: height as usize,
            transform,
            nodata,
            compression,
            little_endian,
            chunk_width: if tiled {
                chunk_width as usize
            } else {
                width as usize
            },
            chunk_height: chunk_height as usize,
            tiled,
            offsets,
            lengths,
        })
    }

    /// Rows `row0 .. row0 + rows` of the band, row-major, reading only the chunks they cross.
    pub fn window(&mut self, row0: usize, rows: usize) -> Result<Vec<f32>, String> {
        if row0 + rows > self.height {
            return Err(format!(
                "{}: rows {row0}..{} outside {} rows",
                self.origin,
                row0 + rows,
                self.height
            ));
        }
        let mut out = vec![0.0f32; rows * self.width];
        if rows == 0 {
            return Ok(out);
        }
        let across = self.width.div_ceil(self.chunk_width);
        for chunk_row in row0 / self.chunk_height..=(row0 + rows - 1) / self.chunk_height {
            let top = chunk_row * self.chunk_height;
            // A tile always holds full chunk rows; the last strip holds only the image's rows.
            let held = if self.tiled {
                self.chunk_height
            } else {
                self.chunk_height.min(self.height - top)
            };
            for chunk_col in 0..across {
                let values = self.chunk(chunk_row * across + chunk_col, held)?;
                let left = chunk_col * self.chunk_width;
                let used = self.chunk_width.min(self.width - left);
                for r in top.max(row0)..(top + held).min(row0 + rows) {
                    let src = (r - top) * self.chunk_width;
                    let dst = (r - row0) * self.width + left;
                    out[dst..dst + used].copy_from_slice(&values[src..src + used]);
                }
            }
        }
        Ok(out)
    }

    /// The values of chunk `index`, `rows` full chunk rows.
    fn chunk(&mut self, index: usize, rows: usize) -> Result<Vec<f32>, String> {
        let fail = |e: &dyn std::fmt::Display| format!("{} chunk {index}: {e}", self.origin);
        let (Some(&offset), Some(&length)) = (self.offsets.get(index), self.lengths.get(index))
        else {
            return Err(fail(&"missing from the chunk offsets"));
        };
        let mut encoded = vec![0u8; usize::try_from(length).map_err(|e| fail(&e))?];
        let reader = self.decoder.inner();
        reader
            .seek(SeekFrom::Start(offset))
            .and_then(|_| reader.read_exact(&mut encoded))
            .map_err(|e| fail(&e))?;
        let bytes = decode(self.compression, encoded).map_err(|e| fail(&e))?;
        let n = rows * self.chunk_width;
        if bytes.len() < 4 * n {
            return Err(fail(&format!("{} bytes, {} expected", bytes.len(), 4 * n)));
        }
        let word = |b: &[u8]| -> [u8; 4] { [b[0], b[1], b[2], b[3]] };
        Ok(bytes[..4 * n]
            .chunks_exact(4)
            .map(|b| {
                if self.little_endian {
                    f32::from_le_bytes(word(b))
                } else {
                    f32::from_be_bytes(word(b))
                }
            })
            .collect())
    }
}

/// The decompressed bytes of one chunk. An LZW chunk may end without its end-of-information code,
/// as libtiff accepts.
fn decode(compression: Compression, encoded: Vec<u8>) -> Result<Vec<u8>, String> {
    match compression {
        Compression::None => Ok(encoded),
        Compression::Lzw => {
            let mut out = Vec::new();
            weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
                .into_vec(&mut out)
                .decode(&encoded)
                .status
                .map_err(|e| e.to_string())?;
            Ok(out)
        }
        Compression::Deflate => {
            let mut out = Vec::new();
            flate2::read::ZlibDecoder::new(&encoded[..])
                .read_to_end(&mut out)
                .map_err(|e| e.to_string())?;
            Ok(out)
        }
    }
}

/// GDAL's geotransform of a north-up GeoTIFF, in rasterio's order.
fn geotransform<R: Read + Seek>(decoder: &mut Decoder<R>) -> Result<[f64; 6], String> {
    let tag = |d: &mut Decoder<R>, t: Tag| d.find_tag(t).map_err(|e| e.to_string());
    if let Some(keys) = tag(decoder, Tag::GeoKeyDirectoryTag)? {
        let keys = keys.into_u16_vec().map_err(|e| e.to_string())?;
        let point = keys
            .get(4..)
            .unwrap_or_default()
            .chunks_exact(4)
            .any(|k| k[0] == RASTER_TYPE_KEY && k[1] == 0 && k[3] == PIXEL_IS_POINT);
        if point {
            return Err("PixelIsPoint rasters are not supported".into());
        }
    }
    if let Some(m) = tag(decoder, Tag::ModelTransformationTag)? {
        let m = m.into_f64_vec().map_err(|e| e.to_string())?;
        if m.len() != 16 {
            return Err(format!("ModelTransformation has {} values", m.len()));
        }
        return Ok([m[0], m[1], m[3], m[4], m[5], m[7]]);
    }
    let (Some(tie), Some(scale)) = (
        tag(decoder, Tag::ModelTiepointTag)?,
        tag(decoder, Tag::ModelPixelScaleTag)?,
    ) else {
        return Err("no ModelTransformation, ModelTiepoint or ModelPixelScale".into());
    };
    let tie = tie.into_f64_vec().map_err(|e| e.to_string())?;
    let scale = scale.into_f64_vec().map_err(|e| e.to_string())?;
    if tie.len() != 6 || scale.len() < 2 {
        return Err(format!(
            "{} tiepoint and {} pixel scale values; one tiepoint expected",
            tie.len(),
            scale.len()
        ));
    }
    let (a, e) = (scale[0], -scale[1]);
    Ok([a, 0.0, tie[3] - tie[0] * a, 0.0, e, tie[4] - tie[1] * e])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tiff::encoder::{TiffEncoder, colortype::Gray32Float};

    fn striped(width: u32, height: u32, values: &[f32]) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        let mut tiff = TiffEncoder::new(&mut bytes).unwrap();
        let mut image = tiff.new_image::<Gray32Float>(width, height).unwrap();
        image.rows_per_strip(2).unwrap();
        let dir = image.encoder();
        dir.write_tag(Tag::ModelPixelScaleTag, &[0.5f64, 0.25, 0.0][..])
            .unwrap();
        dir.write_tag(
            Tag::ModelTiepointTag,
            &[0.0f64, 0.0, 0.0, -10.0, 20.0, 0.0][..],
        )
        .unwrap();
        dir.write_tag(Tag::GdalNodata, "-9").unwrap();
        image.write_data(values).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn windows_cross_strips_and_georeference_follows_gdal() {
        let values: Vec<f32> = (0..15).map(|v| v as f32).collect();
        let mut r = from_bytes(striped(3, 5, &values), "t.tif").unwrap();
        assert_eq!((r.width, r.height), (3, 5));
        assert_eq!(r.transform, [0.5, 0.0, -10.0, 0.0, -0.25, 20.0]);
        assert_eq!(r.nodata, Some(-9.0));
        assert_eq!(r.window(1, 3).unwrap(), values[3..12]);
        assert_eq!(r.window(4, 1).unwrap(), values[12..15]);
        assert!(r.window(4, 2).is_err());
    }

    #[test]
    fn lzw_and_deflate_chunks_decode() {
        let raw: Vec<u8> = (0..4096u32).flat_map(|v| (v % 7).to_le_bytes()).collect();
        let lzw = weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
            .encode(&raw)
            .unwrap();
        assert_eq!(decode(Compression::Lzw, lzw).unwrap(), raw);
        let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        zlib.write_all(&raw).unwrap();
        assert_eq!(
            decode(Compression::Deflate, zlib.finish().unwrap()).unwrap(),
            raw
        );
    }
}
