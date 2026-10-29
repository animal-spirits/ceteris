//! Numbers printed as Python formats them.

use ceteris_reference::output::json_float;

/// Python's `repr` of a list of ints.
pub fn int_list(v: &[usize]) -> String {
    let items: Vec<String> = v.iter().map(usize::to_string).collect();
    format!("[{}]", items.join(", "))
}

/// `repr` of a finite float, `nan` otherwise.
pub fn num(x: f64) -> String {
    if x.is_finite() {
        json_float(x)
    } else {
        "nan".into()
    }
}

/// Python's `format(x, f".{precision}g")`.
pub fn py_g(x: f64, precision: usize) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf" } else { "-inf" }.into();
    }
    let p = precision.max(1);
    let sci = format!("{:.*e}", p - 1, x);
    let (mantissa, exp) = sci.split_once('e').expect("exponent notation");
    let exp: i32 = exp.parse().expect("an integer exponent");
    let trim = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s.to_owned()
        }
    };
    if -4 <= exp && exp < p as i32 {
        trim(&format!("{:.*}", (p as i32 - 1 - exp) as usize, x))
    } else {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", trim(mantissa), exp.abs())
    }
}

/// Python's `format(x, f".{precision}f")`: Rust's fixed notation, NaN spelled `nan`.
pub fn fixed(x: f64, precision: usize) -> String {
    if x.is_nan() {
        "nan".into()
    } else {
        format!("{x:.precision$}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `format(x, ".4g")` and `format(x, ".3g")` in Python.
    #[test]
    fn py_g_matches_python() {
        for (x, p, want) in [
            (0.0, 4, "0"),
            (-0.0, 4, "-0"),
            (1.0, 4, "1"),
            (2.6155, 4, "2.615"),
            (0.4, 4, "0.4"),
            (12345.0, 4, "1.234e+04"),
            (0.000123456, 4, "0.0001235"),
            (0.0000123456, 4, "1.235e-05"),
            (9.9996, 4, "10"),
            (99995.0, 4, "1e+05"),
            (0.0123, 3, "0.0123"),
            (f64::NAN, 3, "nan"),
            (1e300, 3, "1e+300"),
        ] {
            assert_eq!(py_g(x, p), want, "{x} {p}");
        }
    }

    /// `f"{x:.3f}"` in Python.
    #[test]
    fn fixed_matches_python() {
        for (x, want) in [
            (0.0, "0.000"),
            (-0.0, "-0.000"),
            (12.968844, "12.969"),
            (-0.0004, "-0.000"),
            (f64::NAN, "nan"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
        ] {
            assert_eq!(fixed(x, 3), want, "{x}");
        }
    }

    /// `repr` of a list of ints and of floats.
    #[test]
    fn lists_and_floats_print_as_python() {
        assert_eq!(int_list(&[]), "[]");
        assert_eq!(int_list(&[0, 1, 2]), "[0, 1, 2]");
        assert_eq!(num(0.1), "0.1");
        assert_eq!(num(1e-5), "1e-05");
        assert_eq!(num(f64::NAN), "nan");
    }
}
