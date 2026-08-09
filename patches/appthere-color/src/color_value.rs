//! Color value types representing colors in different color spaces.
//!
//! All value types are immutable after construction. Constructors silently
//! clamp component values to their valid ranges.

/// Clamps a value to the range `[min, max]`.
fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// An RGB color with components in `[0.0, 1.0]`.
///
/// # Examples
///
/// ```
/// use appthere_color::RgbColor;
///
/// let color = RgbColor::new(0.5, 0.8, 0.2);
/// assert_eq!(color.red(), 0.5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RgbColor {
    r: f32,
    g: f32,
    b: f32,
}

impl RgbColor {
    /// Creates a new RGB color, clamping components to `[0.0, 1.0]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::RgbColor;
    ///
    /// let c = RgbColor::new(1.5, -0.1, 0.5);
    /// assert_eq!(c.red(), 1.0);
    /// assert_eq!(c.green(), 0.0);
    /// assert_eq!(c.blue(), 0.5);
    /// ```
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self {
            r: clamp(r, 0.0, 1.0),
            g: clamp(g, 0.0, 1.0),
            b: clamp(b, 0.0, 1.0),
        }
    }

    /// Returns the red component.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::RgbColor;
    /// assert_eq!(RgbColor::new(0.3, 0.0, 0.0).red(), 0.3);
    /// ```
    pub fn red(&self) -> f32 { self.r }

    /// Returns the green component.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::RgbColor;
    /// assert_eq!(RgbColor::new(0.0, 0.7, 0.0).green(), 0.7);
    /// ```
    pub fn green(&self) -> f32 { self.g }

    /// Returns the blue component.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::RgbColor;
    /// assert_eq!(RgbColor::new(0.0, 0.0, 0.9).blue(), 0.9);
    /// ```
    pub fn blue(&self) -> f32 { self.b }

    /// Returns the components as a 3-element array `[r, g, b]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::RgbColor;
    /// let c = RgbColor::new(0.1, 0.2, 0.3);
    /// assert_eq!(c.to_array(), [0.1, 0.2, 0.3]);
    /// ```
    pub fn to_array(&self) -> [f32; 3] { [self.r, self.g, self.b] }
}

/// A CMYK color with components in `[0.0, 1.0]`.
///
/// # Examples
///
/// ```
/// use appthere_color::CmykColor;
///
/// let ink = CmykColor::new(0.0, 0.0, 0.0, 1.0);
/// assert_eq!(ink.key(), 1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CmykColor {
    c: f32,
    m: f32,
    y: f32,
    k: f32,
}

impl CmykColor {
    /// Creates a new CMYK color, clamping components to `[0.0, 1.0]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::CmykColor;
    ///
    /// let c = CmykColor::new(1.5, -0.1, 0.5, 0.0);
    /// assert_eq!(c.cyan(), 1.0);
    /// assert_eq!(c.magenta(), 0.0);
    /// ```
    pub fn new(c: f32, m: f32, y: f32, k: f32) -> Self {
        Self {
            c: clamp(c, 0.0, 1.0),
            m: clamp(m, 0.0, 1.0),
            y: clamp(y, 0.0, 1.0),
            k: clamp(k, 0.0, 1.0),
        }
    }

    /// Returns the cyan component.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::CmykColor;
    /// assert_eq!(CmykColor::new(0.5, 0.0, 0.0, 0.0).cyan(), 0.5);
    /// ```
    pub fn cyan(&self) -> f32 { self.c }

    /// Returns the magenta component.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::CmykColor;
    /// assert_eq!(CmykColor::new(0.0, 0.6, 0.0, 0.0).magenta(), 0.6);
    /// ```
    pub fn magenta(&self) -> f32 { self.m }

    /// Returns the yellow component.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::CmykColor;
    /// assert_eq!(CmykColor::new(0.0, 0.0, 0.4, 0.0).yellow(), 0.4);
    /// ```
    pub fn yellow(&self) -> f32 { self.y }

    /// Returns the key (black) component.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::CmykColor;
    /// assert_eq!(CmykColor::new(0.0, 0.0, 0.0, 1.0).key(), 1.0);
    /// ```
    pub fn key(&self) -> f32 { self.k }

    /// Returns the components as a 4-element array `[c, m, y, k]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::CmykColor;
    /// let color = CmykColor::new(0.1, 0.2, 0.3, 0.4);
    /// assert_eq!(color.to_array(), [0.1, 0.2, 0.3, 0.4]);
    /// ```
    pub fn to_array(&self) -> [f32; 4] { [self.c, self.m, self.y, self.k] }
}

/// A CIE L*a*b* color.
///
/// - L: `[0.0, 100.0]`
/// - a: `[-128.0, 127.0]`
/// - b: `[-128.0, 127.0]`
///
/// # Examples
///
/// ```
/// use appthere_color::LabColor;
///
/// let color = LabColor::new(50.0, 20.0, -30.0);
/// assert_eq!(color.l(), 50.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabColor {
    l: f32,
    a: f32,
    b: f32,
}

impl LabColor {
    /// Creates a new Lab color, clamping components to valid ranges.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::LabColor;
    ///
    /// let c = LabColor::new(110.0, 0.0, 0.0);
    /// assert_eq!(c.l(), 100.0);
    /// ```
    pub fn new(l: f32, a: f32, b: f32) -> Self {
        Self {
            l: clamp(l, 0.0, 100.0),
            a: clamp(a, -128.0, 127.0),
            b: clamp(b, -128.0, 127.0),
        }
    }

    /// Returns the L* (lightness) component.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::LabColor;
    /// assert_eq!(LabColor::new(75.0, 0.0, 0.0).l(), 75.0);
    /// ```
    pub fn l(&self) -> f32 { self.l }

    /// Returns the a* component.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::LabColor;
    /// assert_eq!(LabColor::new(0.0, 10.0, 0.0).a(), 10.0);
    /// ```
    pub fn a(&self) -> f32 { self.a }

    /// Returns the b* component.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::LabColor;
    /// assert_eq!(LabColor::new(0.0, 0.0, -50.0).b(), -50.0);
    /// ```
    pub fn b(&self) -> f32 { self.b }

    /// Returns the components as a 3-element array `[l, a, b]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use appthere_color::LabColor;
    /// let c = LabColor::new(50.0, 10.0, -20.0);
    /// assert_eq!(c.to_array(), [50.0, 10.0, -20.0]);
    /// ```
    pub fn to_array(&self) -> [f32; 3] { [self.l, self.a, self.b] }
}
