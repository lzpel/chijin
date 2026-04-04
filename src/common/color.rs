#[cfg(feature = "color")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
	pub r: f32,
	pub g: f32,
	pub b: f32,
}

#[cfg(feature = "color")]
impl std::str::FromStr for Color {
	type Err = super::error::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::from_str(s)
	}
}

#[cfg(feature = "color")]
impl Color {
	/// Parse a color string: CSS named colors (e.g. `"red"`) or hex (`"#f00"`, `"#ff0000"`).
	pub fn from_str(s: &str) -> Result<Self, super::error::Error> {
		if s.starts_with('#') {
			return Self::from_hex(s);
		}
		let hex = match s.to_ascii_lowercase().as_str() {
			"black"               => "#000",
			"white"               => "#fff",
			"red"                 => "#f00",
			"lime"                => "#0f0",
			"blue"                => "#00f",
			"yellow"              => "#ff0",
			"cyan" | "aqua"       => "#0ff",
			"magenta" | "fuchsia" => "#f0f",
			"silver"              => "#c0c0c0",
			"gray" | "grey"       => "#808080",
			"maroon"              => "#800000",
			"olive"               => "#808000",
			"green"               => "#008000",
			"purple"              => "#800080",
			"teal"                => "#008080",
			"navy"                => "#000080",
			"orange"              => "#ffa500",
			"coral"               => "#ff7f50",
			"tomato"              => "#ff6347",
			"salmon"              => "#fa8072",
			"gold"                => "#ffd700",
			"pink"                => "#ffc0cb",
			"violet"              => "#ee82ee",
			"indigo"              => "#4b0082",
			"brown"               => "#a52a2a",
			"tan"                 => "#d2b48c",
			"skyblue"             => "#87ceeb",
			_ => return Err(super::error::Error::InvalidColor(s.to_string())),
		};
		Self::from_hex(hex)
	}

	/// Parse a hex color string like `"#ff8800"` or `"#f80"`.
	///
	/// The leading `#` is required. The remaining characters must be hex digits,
	/// either 6 (RRGGBB) or 3 (RGB, each digit is doubled).
	fn from_hex(s: &str) -> Result<Self, super::error::Error> {
		let err = || super::error::Error::InvalidColor(s.to_string());
		let hex = s.strip_prefix('#').ok_or_else(err)?;
		if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
			return Err(super::error::Error::InvalidColor(s.to_string()));
		}
		let (r, g, b) = match hex.len() {
			6 => {
				let r = u8::from_str_radix(&hex[0..2], 16).unwrap();
				let g = u8::from_str_radix(&hex[2..4], 16).unwrap();
				let b = u8::from_str_radix(&hex[4..6], 16).unwrap();
				(r, g, b)
			}
			3 => {
				let r = u8::from_str_radix(&hex[0..1], 16).unwrap() * 17;
				let g = u8::from_str_radix(&hex[1..2], 16).unwrap() * 17;
				let b = u8::from_str_radix(&hex[2..3], 16).unwrap() * 17;
				(r, g, b)
			}
			_ => return Err(super::error::Error::InvalidColor(s.to_string())),
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
