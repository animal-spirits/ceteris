//! GEOS's coverage simplification, `GEOSCoverageSimplifyVW`, which the `geos` crate does not
//! wrap: a collection of polygons in WKB, simplified with Visvalingam-Whyatt so shared edges
//! stay shared. Links GEOS only with the owner-side `geos` feature.

use std::fmt;

/// Where a call failed, with GEOS's message.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// GEOS could not start a context.
    Context,
    /// The input WKB did not parse.
    Read(String),
    /// The coverage simplification raised, as shapely's GEOSException.
    Simplify(String),
    /// The result did not serialise.
    Write(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Context => write!(f, "GEOS_init_r failed"),
            Error::Read(m) => write!(f, "GEOS could not read the WKB: {m}"),
            Error::Simplify(m) => write!(f, "GEOSCoverageSimplifyVW: {m}"),
            Error::Write(m) => write!(f, "GEOS could not write the WKB: {m}"),
        }
    }
}

/// `shapely.coverage_simplify` of the members of a GeometryCollection, WKB in and out: the
/// result is a GeometryCollection of the simplified members in input order.
#[cfg(feature = "geos")]
#[allow(unsafe_code)]
pub fn coverage_simplify_vw(
    wkb: &[u8],
    tolerance: f64,
    preserve_boundary: bool,
) -> Result<Vec<u8>, Error> {
    use std::ffi::{CStr, c_char, c_int, c_void};

    use geos_sys::{
        GEOS_finish_r, GEOS_init_r, GEOSContext_setErrorMessageHandler_r, GEOSCoverageSimplifyVW_r,
        GEOSFree_r, GEOSGeom_destroy_r, GEOSGeomFromWKB_buf_r, GEOSGeomToWKB_buf_r,
    };

    /// Keeps GEOS's last error message in the `String` that `data` points to.
    unsafe extern "C" fn keep(message: *const c_char, data: *mut c_void) {
        // SAFETY: GEOS passes a NUL-terminated message, and `data` is the `String` registered
        // below, which outlives the context.
        unsafe {
            let last = &mut *data.cast::<String>();
            *last = CStr::from_ptr(message).to_string_lossy().into_owned();
        }
    }

    let mut last = String::new();
    // SAFETY: the context is created here, checked for null, and finished last, after every
    // object made with it is freed; its error handler writes only to `last`, which lives until
    // the end of the function. Each GEOS call receives either `wkb`'s pointer and length,
    // valid for the call, or a non-null pointer this function owns. The output buffer is
    // copied out (`size` bytes, as GEOS reports) before `GEOSFree_r` releases it, and each
    // geometry is destroyed once. No pointer escapes the block.
    unsafe {
        let ctx = GEOS_init_r();
        if ctx.is_null() {
            return Err(Error::Context);
        }
        GEOSContext_setErrorMessageHandler_r(ctx, Some(keep), (&raw mut last).cast::<c_void>());
        let input = GEOSGeomFromWKB_buf_r(ctx, wkb.as_ptr(), wkb.len());
        let mut result = Err(Error::Read(std::mem::take(&mut last)));
        let mut output = std::ptr::null_mut();
        let mut buf = std::ptr::null_mut();
        if !input.is_null() {
            output =
                GEOSCoverageSimplifyVW_r(ctx, input, tolerance, c_int::from(preserve_boundary));
            result = Err(Error::Simplify(std::mem::take(&mut last)));
        }
        if !output.is_null() {
            let mut size = 0;
            buf = GEOSGeomToWKB_buf_r(ctx, output, &mut size);
            result = if buf.is_null() {
                Err(Error::Write(std::mem::take(&mut last)))
            } else {
                Ok(std::slice::from_raw_parts(buf, size).to_vec())
            };
        }
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

#[cfg(all(test, feature = "geos"))]
mod tests {
    use super::*;

    #[test]
    fn unreadable_wkb_is_a_read_error_with_geos_s_message() {
        match coverage_simplify_vw(&[1, 2, 3], 0.01, false) {
            Err(Error::Read(m)) => assert!(!m.is_empty()),
            other => panic!("{other:?}"),
        }
    }
}
