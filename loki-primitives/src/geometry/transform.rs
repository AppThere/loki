// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

/// A 2D affine transform.
///
/// Column-major storage: `[a, b, c, d, e, f]` represents:
/// ```text
/// | a  c  e |
/// | b  d  f |
/// | 0  0  1 |
/// ```
///
/// Not generic over unit `U` — transforms operate on dimensionless `f64`
/// coefficients. Callers extract `.value()` from `Length<U>`, transform,
/// and re-wrap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine2 {
    coeffs: [f64; 6],
}

impl Affine2 {
    /// The identity transform.
    pub const IDENTITY: Self = Self {
        coeffs: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    };

    /// Translation transform.
    #[must_use]
    pub fn translation(dx: f64, dy: f64) -> Self {
        Self {
            coeffs: [1.0, 0.0, 0.0, 1.0, dx, dy],
        }
    }

    /// Rotation transform (radians).
    #[must_use]
    pub fn rotation(radians: f64) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self {
            coeffs: [cos, sin, -sin, cos, 0.0, 0.0],
        }
    }

    /// Scaling transform.
    #[must_use]
    pub fn scale(sx: f64, sy: f64) -> Self {
        Self {
            coeffs: [sx, 0.0, 0.0, sy, 0.0, 0.0],
        }
    }

    /// Uniform scaling transform.
    #[must_use]
    pub fn uniform_scale(s: f64) -> Self {
        Self::scale(s, s)
    }

    /// Compose two transforms: self then other.
    #[must_use]
    pub fn then(self, other: Self) -> Self {
        let [a1, b1, c1, d1, e1, f1] = self.coeffs;
        let [a2, b2, c2, d2, e2, f2] = other.coeffs;

        Self {
            coeffs: [
                a1 * a2 + b1 * c2,
                a1 * b2 + b1 * d2,
                c1 * a2 + d1 * c2,
                c1 * b2 + d1 * d2,
                e1 * a2 + f1 * c2 + e2,
                e1 * b2 + f1 * d2 + f2,
            ],
        }
    }

    /// The inverse of this transform, or None if singular.
    #[must_use]
    pub fn inverse(self) -> Option<Self> {
        let [a, b, c, d, e, f] = self.coeffs;
        let det = a * d - b * c;
        if det.abs() < 1e-6 {
            return None;
        }

        let inv_det = 1.0 / det;
        Some(Self {
            coeffs: [
                d * inv_det,
                -b * inv_det,
                -c * inv_det,
                a * inv_det,
                (c * f - d * e) * inv_det,
                (b * e - a * f) * inv_det,
            ],
        })
    }

    /// Transforms a point (`x` and `y`).
    #[must_use]
    pub fn transform_point(self, x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.coeffs;
        (a * x + c * y + e, b * x + d * y + f)
    }

    /// Transforms a size (`w` and `h` vectors).
    #[must_use]
    pub fn transform_size(self, w: f64, h: f64) -> (f64, f64) {
        let [a, b, c, d, _, _] = self.coeffs;
        (a * w + c * h, b * w + d * h)
    }

    /// Checks if it represents identity transform.
    #[must_use]
    pub fn is_identity(self) -> bool {
        self.coeffs == Self::IDENTITY.coeffs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_identity() {
        let t = Affine2::IDENTITY;
        let (x, y) = t.transform_point(3.0, 4.0);
        assert_relative_eq!(x, 3.0);
        assert_relative_eq!(y, 4.0);
    }

    #[test]
    fn test_translation() {
        let t = Affine2::translation(1.0, 2.0);
        let (x, y) = t.transform_point(0.0, 0.0);
        assert_relative_eq!(x, 1.0);
        assert_relative_eq!(y, 2.0);
    }

    #[test]
    fn test_scale() {
        let t = Affine2::scale(2.0, 3.0);
        let (x, y) = t.transform_point(1.0, 1.0);
        assert_relative_eq!(x, 2.0);
        assert_relative_eq!(y, 3.0);
    }

    #[test]
    fn test_rotation() {
        let t = Affine2::rotation(0.0);
        let (x, y) = t.transform_point(1.0, 0.0);
        assert_relative_eq!(x, 1.0);
        assert_relative_eq!(y, 0.0);
        assert!(t.is_identity());
    }

    #[test]
    fn test_inverse() {
        let t = Affine2::scale(2.0, 3.0).then(Affine2::translation(4.0, 5.0));
        let inv = t.inverse().unwrap();
        let point = (1.0, 2.0);
        let transformed = t.transform_point(point.0, point.1);
        let restored = inv.transform_point(transformed.0, transformed.1);
        assert_relative_eq!(point.0, restored.0);
        assert_relative_eq!(point.1, restored.1);

        assert!(Affine2::scale(0.0, 1.0).inverse().is_none());
    }
}
