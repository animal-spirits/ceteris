//! Ramp check (ADR 0221): adjacent classes stay apart under simulated colour-vision
//! deficiencies, and sequential ramps get darker monotonically.

/// Machado, Oliveira and Fernandes (2009), severity 1, on linear RGB.
pub const PROTANOPIA: [[f64; 3]; 3] = [
    [0.152286, 1.052583, -0.204868],
    [0.114503, 0.786281, 0.099216],
    [-0.003882, -0.048116, 1.051998],
];
pub const DEUTERANOPIA: [[f64; 3]; 3] = [
    [0.367322, 0.860646, -0.227968],
    [0.280085, 0.672501, 0.047413],
    [-0.011820, 0.042940, 0.968881],
];
pub const TRITANOPIA: [[f64; 3]; 3] = [
    [1.255528, -0.076749, -0.178779],
    [-0.078411, 0.930809, 0.147602],
    [0.004733, 0.691367, 0.303900],
];
const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
pub const VISIONS: [(&str, [[f64; 3]; 3]); 4] = [
    ("normal", IDENTITY),
    ("protanopia", PROTANOPIA),
    ("deuteranopia", DEUTERANOPIA),
    ("tritanopia", TRITANOPIA),
];

/// Smallest CIEDE2000 distance between adjacent classes under any vision.
pub const MIN_ADJACENT: f64 = 5.0;

fn linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// CIELAB (D65) of an sRGB colour seen with `vision`.
pub fn lab(rgb: [u8; 3], vision: &[[f64; 3]; 3]) -> [f64; 3] {
    let l = rgb.map(linear);
    let [r, g, b] =
        vision.map(|row| (row[0] * l[0] + row[1] * l[1] + row[2] * l[2]).clamp(0.0, 1.0));
    let x = (0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047;
    let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
    let z = (0.0193339 * r + 0.1191920 * g + 0.9503041 * b) / 1.08883;
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 (Sharma, Wu and Dalal 2005).
pub fn ciede2000(a: [f64; 3], b: [f64; 3]) -> f64 {
    let [l1, a1, b1] = a;
    let [l2, a2, b2] = b;
    let c_bar = (a1.hypot(b1) + a2.hypot(b2)) / 2.0;
    let g = 0.5 * (1.0 - (c_bar.powi(7) / (c_bar.powi(7) + 25f64.powi(7))).sqrt());
    let (a1, a2) = (a1 * (1.0 + g), a2 * (1.0 + g));
    let (c1, c2) = (a1.hypot(b1), a2.hypot(b2));
    let hue = |b: f64, a: f64| {
        if a == 0.0 && b == 0.0 {
            0.0
        } else {
            b.atan2(a).to_degrees().rem_euclid(360.0)
        }
    };
    let (h1, h2) = (hue(b1, a1), hue(b2, a2));
    let dl = l2 - l1;
    let dc = c2 - c1;
    let dh = if c1 * c2 == 0.0 {
        0.0
    } else if (h2 - h1).abs() <= 180.0 {
        h2 - h1
    } else if h2 <= h1 {
        h2 - h1 + 360.0
    } else {
        h2 - h1 - 360.0
    };
    let dh = 2.0 * (c1 * c2).sqrt() * (dh / 2.0).to_radians().sin();
    let l_bar = (l1 + l2) / 2.0;
    let c_bar = (c1 + c2) / 2.0;
    let h_bar = if c1 * c2 == 0.0 {
        h1 + h2
    } else if (h1 - h2).abs() <= 180.0 {
        (h1 + h2) / 2.0
    } else if h1 + h2 < 360.0 {
        (h1 + h2 + 360.0) / 2.0
    } else {
        (h1 + h2 - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * (h_bar - 30.0).to_radians().cos()
        + 0.24 * (2.0 * h_bar).to_radians().cos()
        + 0.32 * (3.0 * h_bar + 6.0).to_radians().cos()
        - 0.20 * (4.0 * h_bar - 63.0).to_radians().cos();
    let sl = 1.0 + 0.015 * (l_bar - 50.0).powi(2) / (20.0 + (l_bar - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * c_bar;
    let sh = 1.0 + 0.015 * c_bar * t;
    let d_theta = 30.0 * (-((h_bar - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (c_bar.powi(7) / (c_bar.powi(7) + 25f64.powi(7))).sqrt();
    let rt = -(2.0 * d_theta).to_radians().sin() * rc;
    ((dl / sl).powi(2) + (dc / sc).powi(2) + (dh / sh).powi(2) + rt * (dc / sc) * (dh / sh)).sqrt()
}

/// Problems with a ramp of classes ordered low to high; empty when it passes.
pub fn check(ramp: &[[u8; 3]], sequential: bool) -> Vec<String> {
    let mut problems = Vec::new();
    for (vision, m) in &VISIONS {
        let labs: Vec<_> = ramp.iter().map(|&c| lab(c, m)).collect();
        for (k, pair) in labs.windows(2).enumerate() {
            let d = ciede2000(pair[0], pair[1]);
            if d < MIN_ADJACENT {
                problems.push(format!(
                    "{vision}: classes {k} and {} differ by {d:.1}",
                    k + 1
                ));
            }
            if sequential && *vision == "normal" && pair[1][0] >= pair[0][0] {
                problems.push(format!("lightness rises from class {k} to {}", k + 1));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ciede2000_matches_sharma_pairs() {
        let pairs = [
            ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
            ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
            (
                [60.2574, -34.0099, 36.2677],
                [60.4626, -34.1751, 39.4387],
                1.2644,
            ),
            ([2.0776, 0.0795, -1.135], [0.9033, -0.0636, -0.5514], 0.9082),
        ];
        for (a, b, want) in pairs {
            assert!((ciede2000(a, b) - want).abs() < 1e-4, "{a:?} {b:?}");
        }
    }

    #[test]
    fn white_is_lightness_100_under_every_vision() {
        for (_, m) in &VISIONS {
            let [l, _, _] = lab([255, 255, 255], m);
            assert!((l - 100.0).abs() < 0.01, "{l}");
        }
    }

    #[test]
    fn a_grey_ramp_collapses_and_reversed_lightness_is_caught() {
        assert!(!check(&[[100, 100, 100], [101, 101, 101]], true).is_empty());
        assert!(!check(&[[50, 50, 50], [200, 200, 200]], true).is_empty());
        assert!(check(&[[200, 200, 200], [50, 50, 50]], true).is_empty());
    }
}
