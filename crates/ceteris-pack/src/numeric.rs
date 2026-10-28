//! Floating point operations with the summation order and rounding of named numpy and Python
//! functions (`np.sum`, `math.fsum`, `sum`, `float`, `repr`), bit for bit.

/// `np.sum` of a contiguous float64 array: pairwise summation with eight accumulators in blocks
/// of at most 128 elements.
pub fn pairwise_sum(a: &[f64]) -> f64 {
    0.0 + pairwise(a)
}

fn pairwise(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut res = 0.0;
        for &x in a {
            res += x;
        }
        res
    } else if n <= 128 {
        let mut r = [0.0; 8];
        r.copy_from_slice(&a[..8]);
        let whole = n - n % 8;
        let mut i = 8;
        while i < whole {
            for (j, acc) in r.iter_mut().enumerate() {
                *acc += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        for &x in &a[i..] {
            res += x;
        }
        res
    } else {
        let half = n / 2;
        let half = half - half % 8;
        pairwise(&a[..half]) + pairwise(&a[half..])
    }
}

/// Python's `math.fsum`: the correctly rounded sum (Shewchuk's partials, as CPython keeps them).
pub fn fsum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut partials: Vec<f64> = Vec::new();
    let (mut special, mut inf) = (0.0, 0.0);
    for v in values {
        let mut x = v;
        let mut i = 0;
        for j in 0..partials.len() {
            let mut y = partials[j];
            if x.abs() < y.abs() {
                std::mem::swap(&mut x, &mut y);
            }
            let hi = x + y;
            let lo = y - (hi - x);
            if lo != 0.0 {
                partials[i] = lo;
                i += 1;
            }
            x = hi;
        }
        partials.truncate(i);
        if x != 0.0 {
            if !x.is_finite() {
                if v.is_infinite() {
                    inf += v;
                }
                special += v;
                partials.clear();
            } else {
                partials.push(x);
            }
        }
    }
    if special != 0.0 {
        return if inf.is_nan() { f64::NAN } else { special };
    }
    let Some(mut hi) = partials.pop() else {
        return 0.0;
    };
    let mut lo = 0.0;
    while let Some(y) = partials.pop() {
        let x = hi;
        hi = x + y;
        lo = y - (hi - x);
        if lo != 0.0 {
            break;
        }
    }
    // Half-even rounding across the partials left below the break.
    if let Some(&next) = partials.last()
        && ((lo < 0.0 && next < 0.0) || (lo > 0.0 && next > 0.0))
    {
        let y = lo * 2.0;
        let x = hi + y;
        if y == x - hi {
            hi = x;
        }
    }
    hi
}

/// Python's built-in `sum` of floats from the integer start 0: Neumaier's compensated sum.
pub fn py_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut values = values.into_iter();
    let Some(first) = values.next() else {
        return 0.0;
    };
    let (mut sum, mut c) = (0.0 + first, 0.0);
    for x in values {
        let t = sum + x;
        if sum.abs() >= x.abs() {
            c += (sum - t) + x;
        } else {
            c += (x - t) + sum;
        }
        sum = t;
    }
    if c != 0.0 && c.is_finite() {
        sum += c;
    }
    sum
}

/// Python's `float(text)`: surrounding whitespace, `_` between digits, `inf` and `nan` in any case.
pub fn parse_f64(text: &str) -> Option<f64> {
    let t = text.trim();
    if t.contains('_') {
        let b = t.as_bytes();
        let ok = b.iter().enumerate().all(|(i, &c)| {
            c != b'_'
                || (i > 0
                    && i + 1 < b.len()
                    && b[i - 1].is_ascii_digit()
                    && b[i + 1].is_ascii_digit())
        });
        return if ok {
            t.replace('_', "").parse().ok()
        } else {
            None
        };
    }
    t.parse().ok()
}

/// Python's `repr` of a float: the shortest digits that round-trip, in fixed notation for
/// exponents from -4 to 15 and in `1e+16` notation otherwise.
pub fn repr_f64(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf" } else { "-inf" }.into();
    }
    // The shortest digit count that round-trips, then the digits of that count nearest the exact
    // value (ties to even), as Python picks among equally short candidates.
    let shortest = format!("{:e}", x.abs());
    let n = shortest
        .split_once('e')
        .expect("exponent notation")
        .0
        .replace('.', "")
        .len();
    let sci = format!("{:.*e}", n - 1, x.abs());
    let (mantissa, exp) = sci.split_once('e').expect("exponent notation");
    let exp: i32 = exp.parse().expect("exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let sign = if x.is_sign_negative() { "-" } else { "" };
    if !(-4..16).contains(&exp) {
        let (head, tail) = digits.split_at(1);
        let dot = if tail.is_empty() {
            String::new()
        } else {
            format!(".{tail}")
        };
        let esign = if exp < 0 { '-' } else { '+' };
        return format!("{sign}{head}{dot}e{esign}{:02}", exp.abs());
    }
    let point = exp + 1;
    let body = if point <= 0 {
        format!("0.{}{digits}", "0".repeat(point.unsigned_abs() as usize))
    } else if point as usize >= digits.len() {
        format!("{digits}{}.0", "0".repeat(point as usize - digits.len()))
    } else {
        let (int, frac) = digits.split_at(point as usize);
        format!("{int}.{frac}")
    };
    format!("{sign}{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repr_follows_python() {
        let cases = [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (0.1, "0.1"),
            (2000.0, "2000.0"),
            (1e16, "1e+16"),
            (1e15, "1000000000000000.0"),
            (123456789012345680.0, "1.2345678901234568e+17"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (-2.5e300, "-2.5e+300"),
            (36526.0, "36526.0"),
            (0.30000000000000004, "0.30000000000000004"),
            (22394438973027.0 + 0.3125, "22394438973027.312"), // a tie: the even digit
        ];
        for (x, want) in cases {
            assert_eq!(repr_f64(x), want, "{x:e}");
        }
    }

    #[test]
    fn parse_follows_python_float() {
        assert_eq!(parse_f64(" 1.5 "), Some(1.5));
        assert_eq!(parse_f64("1_000"), Some(1000.0));
        assert_eq!(parse_f64("1__0"), None);
        assert_eq!(parse_f64("_1"), None);
        assert_eq!(parse_f64("-Infinity"), Some(f64::NEG_INFINITY));
        assert!(parse_f64("nan").unwrap().is_nan());
        assert_eq!(parse_f64("TBD"), None);
        assert_eq!(parse_f64(""), None);
        assert_eq!(parse_f64("1e3"), Some(1000.0));
    }

    #[test]
    fn fsum_is_exactly_rounded() {
        assert_eq!(fsum([0.1; 10]), 1.0);
        assert_eq!(
            fsum([1e100, 1.0, -1e100, 1e-100, 1e50, -1.0, -1e50]),
            1e-100
        );
        assert_eq!(fsum([]), 0.0);
        // Half-even across partials: CPython's test case.
        assert_eq!(
            fsum([1.0, 2f64.powi(-53), 2f64.powi(-106)]),
            1.0 + 2f64.powi(-52)
        );
    }

    #[test]
    fn py_sum_compensates() {
        assert_eq!(py_sum([0.1; 10]), 1.0);
        assert_eq!(py_sum([1e16, 1.0, -1e16]), 1.0);
    }

    #[test]
    fn pairwise_matches_numpy_blocking() {
        // Sequential below 8 elements; eight lanes up to 128.
        let a: Vec<f64> = (0..10).map(|i| 0.1 * f64::from(i)).collect();
        let r: Vec<f64> = (0..8).map(|j| a[j]).collect();
        let want =
            (((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]))) + a[8] + a[9];
        assert_eq!(pairwise_sum(&a), want);
        assert_eq!(pairwise_sum(&[1.0, 2.0, 3.0]), 6.0);
        assert_eq!(pairwise_sum(&[]), 0.0);
    }
}
