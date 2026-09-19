use crate::traits::app_runtime::DesktopBannerBackground;

impl DesktopBannerBackground {
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty() || value.eq_ignore_ascii_case("system") {
            return Ok(Self::System);
        }
        if value.eq_ignore_ascii_case("light") {
            return Ok(Self::Light);
        }
        if value.eq_ignore_ascii_case("dark") {
            return Ok(Self::Dark);
        }
        parse_css_color(value)
            .map(|(r, g, b, a)| Self::Color { r, g, b, a })
            .ok_or_else(|| {
                "banner background must be system, light, dark, or #RGB/#RRGGBB/#RRGGBBAA".into()
            })
    }

    pub fn as_ffi(&self) -> String {
        match self {
            Self::System => String::new(),
            Self::Light => "light".into(),
            Self::Dark => "dark".into(),
            Self::Color { r, g, b, a } if *a == 255 => format!("#{r:02x}{g:02x}{b:02x}"),
            Self::Color { r, g, b, a } => format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
        }
    }

    pub fn prefers_dark_content(&self) -> bool {
        match self {
            Self::Light => true,
            Self::Dark | Self::System => false,
            Self::Color { r, g, b, .. } => relative_luminance(*r, *g, *b) > 0.55,
        }
    }
}

fn parse_css_color(value: &str) -> Option<(u8, u8, u8, u8)> {
    let hex = value.strip_prefix('#')?;
    let digits = hex.as_bytes();
    let nibble = |byte| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    };
    let pair = |hi, lo| Some(nibble(hi)? * 16 + nibble(lo)?);
    match digits {
        [r, g, b] => Some((nibble(*r)? * 17, nibble(*g)? * 17, nibble(*b)? * 17, 255)),
        [r1, r2, g1, g2, b1, b2] => Some((pair(*r1, *r2)?, pair(*g1, *g2)?, pair(*b1, *b2)?, 255)),
        [r1, r2, g1, g2, b1, b2, a1, a2] => Some((
            pair(*r1, *r2)?,
            pair(*g1, *g2)?,
            pair(*b1, *b2)?,
            pair(*a1, *a2)?,
        )),
        _ => None,
    }
}

fn relative_luminance(r: u8, g: u8, b: u8) -> f32 {
    0.2126 * (r as f32 / 255.0) + 0.7152 * (g as f32 / 255.0) + 0.0722 * (b as f32 / 255.0)
}
