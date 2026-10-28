//! GEOS's coverage simplification, `GEOSCoverageSimplifyVW`, which the `geos` crate does not
//! wrap: a collection of polygons in WKB, simplified with Visvalingam-Whyatt so shared edges
//! stay shared. Links GEOS only with the owner-side `geos` feature.

/// `shapely.coverage_simplify` of the members of a GeometryCollection, WKB in and out: the
/// result is a GeometryCollection of the simplified members in input order. Fails where GEOS
/// raises (for one, on a collection that is not a valid coverage).
#[cfg(feature = "geos")]
#[allow(unsafe_code)]
pub fn coverage_simplify_vw(
    wkb: &[u8],
    tolerance: f64,
    preserve_boundary: bool,
) -> Result<Vec<u8>, String> {
    use std::ffi::c_int;

    use geos_sys::{
        GEOS_finish_r, GEOS_init_r, GEOSCoverageSimplifyVW_r, GEOSFree_r, GEOSGeom_destroy_r,
        GEOSGeomFromWKB_buf_r, GEOSGeomToWKB_buf_r,
    };
    // SAFETY: the context is created here, checked for null, and finished last, after every
    // object made with it is freed. Each GEOS call receives either `wkb`'s pointer and length,
    // valid for the call, or a non-null pointer this function owns. The output buffer is
    // copied out (`size` bytes, as GEOS reports) before `GEOSFree_r` releases it, and each
    // geometry is destroyed once. No pointer escapes the block.
    unsafe {
        let ctx = GEOS_init_r();
        if ctx.is_null() {
            return Err("GEOS_init_r failed".into());
        }
        let input = GEOSGeomFromWKB_buf_r(ctx, wkb.as_ptr(), wkb.len());
        let output = if input.is_null() {
            std::ptr::null_mut()
        } else {
            GEOSCoverageSimplifyVW_r(ctx, input, tolerance, c_int::from(preserve_boundary))
        };
        let mut size = 0;
        let buf = if output.is_null() {
            std::ptr::null_mut()
        } else {
            GEOSGeomToWKB_buf_r(ctx, output, &mut size)
        };
        let result = if input.is_null() {
            Err("GEOS could not read the WKB".to_owned())
        } else if output.is_null() {
            Err("GEOSCoverageSimplifyVW failed".to_owned())
        } else if buf.is_null() {
            Err("GEOS could not write the WKB".to_owned())
        } else {
            Ok(std::slice::from_raw_parts(buf, size).to_vec())
        };
        if !buf.is_null() {
            GEOSFree_r(ctx, buf.cast());
        }
        if !output.is_null() {
            GEOSGeom_destroy_r(ctx, output);
        }
        if !input.is_null() {
            GEOSGeom_destroy_r(ctx, input);
        }
        GEOS_finish_r(ctx);
        result
    }
}
