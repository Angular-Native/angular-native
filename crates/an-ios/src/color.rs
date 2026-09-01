//! Colores: de la cadena que manda el estilo a `UIColor`.
//!
//! Se acepta lo mismo que en web (`#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`,
//! `rgba()`) más un puñado de nombres. No hay `currentColor` ni cascada.

/// RGBA en 0..1.
pub type Rgba = (f64, f64, f64, f64);

pub fn parse(raw: &str) -> Option<Rgba> {
    let raw = raw.trim();
    if let Some(hex) = raw.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(rest) = raw.strip_prefix("rgba(").and_then(|r| r.strip_suffix(')')) {
        return parse_channels(rest, true);
    }
    if let Some(rest) = raw.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        return parse_channels(rest, false);
    }
    match raw {
        "transparent" => Some((0.0, 0.0, 0.0, 0.0)),
        "black" => Some((0.0, 0.0, 0.0, 1.0)),
        "white" => Some((1.0, 1.0, 1.0, 1.0)),
        "red" => Some((1.0, 0.0, 0.0, 1.0)),
        "green" => Some((0.0, 0.5, 0.0, 1.0)),
        "blue" => Some((0.0, 0.0, 1.0, 1.0)),
        "gray" | "grey" => Some((0.5, 0.5, 0.5, 1.0)),
        _ => None,
    }
}

fn parse_hex(hex: &str) -> Option<Rgba> {
    let nibble = |c: char| c.to_digit(16).map(|d| d as f64);
    let byte = |s: &str| u8::from_str_radix(s, 16).ok().map(|v| v as f64 / 255.0);
    match hex.len() {
        3 | 4 => {
            let mut chars = hex.chars();
            let r = nibble(chars.next()?)? / 15.0;
            let g = nibble(chars.next()?)? / 15.0;
            let b = nibble(chars.next()?)? / 15.0;
            let a = match chars.next() {
                Some(c) => nibble(c)? / 15.0,
                None => 1.0,
            };
            Some((r, g, b, a))
        }
        6 | 8 => {
            let r = byte(&hex[0..2])?;
            let g = byte(&hex[2..4])?;
            let b = byte(&hex[4..6])?;
            let a = if hex.len() == 8 { byte(&hex[6..8])? } else { 1.0 };
            Some((r, g, b, a))
        }
        _ => None,
    }
}

fn parse_channels(body: &str, with_alpha: bool) -> Option<Rgba> {
    let mut parts = body.split(',').map(str::trim);
    let channel = |s: Option<&str>| -> Option<f64> {
        let s = s?;
        match s.strip_suffix('%') {
            Some(p) => p.trim().parse::<f64>().ok().map(|v| v / 100.0),
            None => s.parse::<f64>().ok().map(|v| v / 255.0),
        }
    };
    let r = channel(parts.next())?;
    let g = channel(parts.next())?;
    let b = channel(parts.next())?;
    let a = if with_alpha {
        parts.next()?.parse::<f64>().ok()?
    } else {
        1.0
    };
    Some((r, g, b, a))
}

/// Blanco o negro, el que se lea encima del color que se le pase.
///
/// La luminancia va con los pesos de siempre —el ojo ve mucho más el verde que
/// el azul—, y el corte en 0,55 es el que deja el texto legible tanto sobre un
/// amarillo como sobre un azul marino.
pub fn contrast_on(color: Rgba) -> Rgba {
    let (r, g, b, _) = color;
    let luminance = 0.299 * r + 0.587 * g + 0.114 * b;
    if luminance > 0.55 { (0.0, 0.0, 0.0, 1.0) } else { (1.0, 1.0, 1.0, 1.0) }
}

#[cfg(any(target_os = "ios", target_os = "tvos", target_os = "visionos"))]
pub fn to_uicolor(raw: &str) -> Option<objc2::rc::Retained<objc2_ui_kit::UIColor>> {
    let (r, g, b, a) = parse(raw)?;
    Some(objc2_ui_kit::UIColor::colorWithRed_green_blue_alpha(r, g, b, a))
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn acepta_las_formas_de_css() {
        assert_eq!(parse("#fff"), Some((1.0, 1.0, 1.0, 1.0)));
        assert_eq!(parse("#00ff00"), Some((0.0, 1.0, 0.0, 1.0)));
        assert_eq!(parse("#00000080").map(|c| (c.3 * 255.0).round()), Some(128.0));
        assert_eq!(parse("rgb(255, 0, 0)"), Some((1.0, 0.0, 0.0, 1.0)));
        assert_eq!(parse("rgba(0, 0, 0, 0.5)"), Some((0.0, 0.0, 0.0, 0.5)));
        assert_eq!(parse("transparent"), Some((0.0, 0.0, 0.0, 0.0)));
        assert_eq!(parse("no-es-un-color"), None);
    }
}
