#[cfg(feature = "color")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
	pub r: f32,
	pub g: f32,
	pub b: f32,
}

#[cfg(feature = "color")]
impl Color {
	/// Create an `Color` from HSV values (all in `0.0..=1.0`).
	/// Parse a hex color string like `"#ff8800"` or `"#f80"`.
	///
	/// The leading `#` is required. The remaining characters must be hex digits,
	/// either 6 (RRGGBB) or 3 (RGB, each digit is doubled).
	pub fn from_hex(s: &str) -> Result<Self, crate::error::Error> {
		let s = s.strip_prefix('#').ok_or(crate::error::Error::InvalidHexColor)?;
		if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
			return Err(crate::error::Error::InvalidHexColor);
		}
		let (r, g, b) = match s.len() {
			6 => {
				let r = u8::from_str_radix(&s[0..2], 16).unwrap();
				let g = u8::from_str_radix(&s[2..4], 16).unwrap();
				let b = u8::from_str_radix(&s[4..6], 16).unwrap();
				(r, g, b)
			}
			3 => {
				let r = u8::from_str_radix(&s[0..1], 16).unwrap() * 17;
				let g = u8::from_str_radix(&s[1..2], 16).unwrap() * 17;
				let b = u8::from_str_radix(&s[2..3], 16).unwrap() * 17;
				(r, g, b)
			}
			_ => return Err(crate::error::Error::InvalidHexColor),
		};
		Ok(Color {
			r: r as f32 / 255.0,
			g: g as f32 / 255.0,
			b: b as f32 / 255.0,
		})
	}

	/// Create an `Color` from HSV values (all in `0.0..=1.0`).
	pub fn from_hsv(h: f32, s: f32, v: f32) -> Self {
		let h6 = h * 6.0;
		let f = h6.fract();
		let p = v * (1.0 - s);
		let q = v * (1.0 - s * f);
		let t = v * (1.0 - s * (1.0 - f));
		let (r, g, b) = match h6 as u32 % 6 {
			0 => (v, t, p),
			1 => (q, v, p),
			2 => (p, v, t),
			3 => (p, q, v),
			4 => (t, p, v),
			_ => (v, p, q),
		};
		Color { r, g, b }
	}
}
