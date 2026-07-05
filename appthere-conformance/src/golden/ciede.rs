// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! sRGB → CIELAB (D65) conversion and the CIEDE2000 colour difference
//! (Sharma et al. 2005 formulation), split from `diff.rs` to keep files
//! under the 300-line ceiling. Verified against the published CIEDE2000
//! reference pairs in `diff_tests.rs`.

/// Converts an 8-bit sRGB pixel to CIELAB (D65 white point).
pub(super) fn srgb_to_lab([r, g, b, _]: [u8; 4]) -> [f64; 3] {
    fn linear(c: u8) -> f64 {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    let (rl, gl, bl) = (linear(r), linear(g), linear(b));
    // sRGB D65 → XYZ.
    let x = 0.4124564 * rl + 0.3575761 * gl + 0.1804375 * bl;
    let y = 0.2126729 * rl + 0.7151522 * gl + 0.0721750 * bl;
    let z = 0.0193339 * rl + 0.1191920 * gl + 0.9503041 * bl;
    // XYZ → Lab with the D65 reference white.
    fn f(t: f64) -> f64 {
        const DELTA: f64 = 6.0 / 29.0;
        if t > DELTA * DELTA * DELTA {
            t.cbrt()
        } else {
            t / (3.0 * DELTA * DELTA) + 4.0 / 29.0
        }
    }
    let (xn, yn, zn) = (0.95047, 1.0, 1.08883);
    let (fx, fy, fz) = (f(x / xn), f(y / yn), f(z / zn));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 colour difference (Sharma et al. 2005 formulation).
#[allow(clippy::many_single_char_names, clippy::similar_names)]
pub(super) fn ciede2000([l1, a1, b1]: [f64; 3], [l2, a2, b2]: [f64; 3]) -> f64 {
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let c_bar = (c1 + c2) / 2.0;
    let c_bar7 = c_bar.powi(7);
    let g = 0.5 * (1.0 - (c_bar7 / (c_bar7 + 25.0_f64.powi(7))).sqrt());
    let ap1 = (1.0 + g) * a1;
    let ap2 = (1.0 + g) * a2;
    let cp1 = (ap1 * ap1 + b1 * b1).sqrt();
    let cp2 = (ap2 * ap2 + b2 * b2).sqrt();
    let hp = |ap: f64, b: f64| -> f64 {
        if ap == 0.0 && b == 0.0 {
            0.0
        } else {
            let h = b.atan2(ap).to_degrees();
            if h < 0.0 { h + 360.0 } else { h }
        }
    };
    let hp1 = hp(ap1, b1);
    let hp2 = hp(ap2, b2);

    let dl = l2 - l1;
    let dc = cp2 - cp1;
    let dhp = if cp1 * cp2 == 0.0 {
        0.0
    } else {
        let d = hp2 - hp1;
        if d.abs() <= 180.0 {
            d
        } else if d > 180.0 {
            d - 360.0
        } else {
            d + 360.0
        }
    };
    let dh = 2.0 * (cp1 * cp2).sqrt() * (dhp.to_radians() / 2.0).sin();

    let l_bar = (l1 + l2) / 2.0;
    let cp_bar = (cp1 + cp2) / 2.0;
    let hp_bar = if cp1 * cp2 == 0.0 {
        hp1 + hp2
    } else {
        let sum = hp1 + hp2;
        let d = (hp1 - hp2).abs();
        if d <= 180.0 {
            sum / 2.0
        } else if sum < 360.0 {
            (sum + 360.0) / 2.0
        } else {
            (sum - 360.0) / 2.0
        }
    };

    let t = 1.0 - 0.17 * (hp_bar - 30.0).to_radians().cos()
        + 0.24 * (2.0 * hp_bar).to_radians().cos()
        + 0.32 * (3.0 * hp_bar + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hp_bar - 63.0).to_radians().cos();
    let d_theta = 30.0 * (-((hp_bar - 275.0) / 25.0).powi(2)).exp();
    let cp_bar7 = cp_bar.powi(7);
    let rc = 2.0 * (cp_bar7 / (cp_bar7 + 25.0_f64.powi(7))).sqrt();
    let lb50 = (l_bar - 50.0).powi(2);
    let sl = 1.0 + 0.015 * lb50 / (20.0 + lb50).sqrt();
    let sc = 1.0 + 0.045 * cp_bar;
    let sh = 1.0 + 0.015 * cp_bar * t;
    let rt = -(2.0 * d_theta).to_radians().sin() * rc;

    ((dl / sl).powi(2) + (dc / sc).powi(2) + (dh / sh).powi(2) + rt * (dc / sc) * (dh / sh)).sqrt()
}
