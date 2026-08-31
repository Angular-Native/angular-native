//! Medición de nodos hoja. El layout no puede resolver un `<Text>` sin
//! preguntar a alguien cuánto ocupa ese texto con esa fuente.
//!
//! En iOS/Android lo responde la plataforma (UIKit / android.text). En tests y
//! en CI responde `NaiveMeasurer`, que aproxima sin dependencias de sistema.

/// Fuente con la que se mide un `<Text>`.
#[derive(Clone, Debug, PartialEq)]
pub struct FontSpec {
    pub size: f32,
    /// 100..900, escala CSS.
    pub weight: u16,
    pub italic: bool,
    pub family: Option<String>,
    /// Alto de línea absoluto en puntos. `None` = derivado del tamaño.
    pub line_height: Option<f32>,
    pub letter_spacing: f32,
    /// Truncado a N líneas. `None` = sin límite.
    pub max_lines: Option<u32>,
}

impl Default for FontSpec {
    fn default() -> Self {
        FontSpec {
            size: 14.0,
            weight: 400,
            italic: false,
            family: None,
            line_height: None,
            letter_spacing: 0.0,
            max_lines: None,
        }
    }
}

/// Qué hace falta medir en un nodo hoja.
#[derive(Clone, Debug)]
pub enum MeasureCtx {
    Text { text: String, font: FontSpec },
    /// Imagen con tamaño intrínseco conocido (ancho, alto).
    Image { intrinsic: (f32, f32) },
}

/// Lo implementa cada plataforma. Debe ser puro: mismas entradas, misma salida,
/// o el layout oscila entre frames.
pub trait TextMeasurer {
    /// `max_width` = `None` cuando el ancho disponible es infinito.
    /// Devuelve (ancho, alto) en puntos lógicos.
    fn measure_text(&self, text: &str, font: &FontSpec, max_width: Option<f32>) -> (f32, f32);
}

/// Aproximación monoespaciada. Sirve para tests y para no bloquear el núcleo
/// mientras la capa nativa no existe. No usar en producción.
#[derive(Clone, Copy, Debug, Default)]
pub struct NaiveMeasurer;

impl NaiveMeasurer {
    const ADVANCE_RATIO: f32 = 0.55;
    const LINE_RATIO: f32 = 1.25;
}

impl TextMeasurer for NaiveMeasurer {
    fn measure_text(&self, text: &str, font: &FontSpec, max_width: Option<f32>) -> (f32, f32) {
        let advance = font.size * Self::ADVANCE_RATIO + font.letter_spacing;
        let line_height = font.line_height.unwrap_or(font.size * Self::LINE_RATIO);
        let char_width = |s: &str| s.chars().count() as f32 * advance;

        let Some(limit) = max_width.filter(|w| w.is_finite() && *w > 0.0) else {
            let widest = text.lines().map(char_width).fold(0.0_f32, f32::max);
            let lines = text.lines().count().max(1) as f32;
            return (widest, lines * line_height);
        };

        // Salto por palabras; una palabra más ancha que el límite desborda,
        // no se parte. Es lo que hace UIKit por defecto.
        let mut lines = 0_u32;
        let mut widest = 0.0_f32;
        for paragraph in text.split('\n') {
            let mut current = 0.0_f32;
            let mut started = false;
            for word in paragraph.split_whitespace() {
                let w = char_width(word);
                let candidate = if started { current + advance + w } else { w };
                if started && candidate > limit {
                    widest = widest.max(current);
                    lines += 1;
                    current = w;
                } else {
                    current = candidate;
                    started = true;
                }
            }
            widest = widest.max(current);
            lines += 1;
        }

        let lines = match font.max_lines {
            Some(max) if max > 0 => lines.min(max),
            _ => lines,
        };
        (widest.min(limit), lines.max(1) as f32 * line_height)
    }
}
