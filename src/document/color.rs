/// Parse `#rgb`, `#rrggbb`, `rgb`, or `rrggbb` into an RGB triple.
pub fn parse_hex_color(value: &str) -> Option<(u8, u8, u8)> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            Some((r * 17, g * 17, b * 17))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}

pub fn to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

pub fn normalize_hex_color(value: &str) -> Option<String> {
    parse_hex_color(value).map(|(r, g, b)| to_hex(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_long() {
        assert_eq!(parse_hex_color("#12ab34"), Some((0x12, 0xab, 0x34)));
        assert_eq!(parse_hex_color("12ab34"), Some((0x12, 0xab, 0x34)));
    }

    #[test]
    fn parse_short() {
        assert_eq!(parse_hex_color("#f00"), Some((255, 0, 0)));
    }

    #[test]
    fn reject_invalid() {
        assert_eq!(parse_hex_color(""), None);
        assert_eq!(parse_hex_color("#12345"), None);
        assert_eq!(parse_hex_color("#gggggg"), None);
    }

    #[test]
    fn normalize() {
        assert_eq!(normalize_hex_color("#F0A").as_deref(), Some("#ff00aa"));
    }
}
