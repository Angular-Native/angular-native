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
    /// Un control del sistema —un interruptor, un deslizador, una barra de
    /// pestañas—. Cuánto mide no lo decide el framework: lo decide la
    /// plataforma, y cambia entre versiones del sistema.
    Control { name: String },
}

/// Lo implementa cada plataforma. Debe ser puro: mismas entradas, misma salida,
/// o el layout oscila entre frames.
pub trait TextMeasurer {
    /// `max_width` = `None` cuando el ancho disponible es infinito.
    /// Devuelve (ancho, alto) en puntos lógicos.
    fn measure_text(&self, text: &str, font: &FontSpec, max_width: Option<f32>) -> (f32, f32);

    /// El mínimo intrínseco de un texto: lo más estrecho que puede quedarse
    /// sin partir palabras por la mitad, o sea el ancho de la palabra más
    /// larga.
    ///
    /// Hay que preguntarlo aparte y no colarlo como "ancho disponible cero":
    /// medir con ancho cero devuelve cero en UIKit y en Android, y un texto de
    /// mínimo cero se encoge a nada en cuanto su contenedor no le impone un
    /// ancho —dentro de un `alignItems: center`, por ejemplo—. El texto
    /// desaparecía de la pantalla sin que fallara nada.
    fn measure_text_min_content(&self, text: &str, font: &FontSpec) -> (f32, f32) {
        let widest = text
            .split_whitespace()
            .map(|word| self.measure_text(word, font, None).0)
            .fold(0.0_f32, f32::max);
        if widest <= 0.0 {
            // Un texto sin espacios —o vacío— no tiene nada que partir: su
            // mínimo es su tamaño entero.
            return self.measure_text(text, font, None);
        }
        // El alto es el de ese texto partido a ese ancho, que es más de una
        // línea: el mínimo intrínseco es ancho *y* alto, y quedarse con el
        // alto de una sola línea recortaría el texto.
        let (_, height) = self.measure_text(text, font, Some(widest));
        (widest, height)
    }

    /// Tamaño natural de un control del sistema.
    ///
    /// La implementación por defecto devuelve medidas razonables para que el
    /// núcleo sea usable sin plataforma; cada host la sustituye preguntando al
    /// control de verdad, que es quien sabe cuánto ocupa en esta versión del
    /// sistema y con los ajustes de accesibilidad del usuario.
    fn measure_control(&self, name: &str, _available_width: Option<f32>) -> (f32, f32) {
        match name {
            "Switch" => (51.0, 31.0),
            "Slider" => (200.0, 32.0),
            "ActivityIndicator" => (20.0, 20.0),
            "ProgressBar" => (200.0, 4.0),
            "Button" => (80.0, 44.0),
            "TabBar" => (320.0, 49.0),
            // El tamaño por defecto de un icono. Se puede cambiar con
            // `[size]`, que además de configurar el símbolo fija el ancho y el
            // alto: así un icono sin medidas no queda invisible.
            "Icon" => (24.0, 24.0),
            "SegmentedControl" => (320.0, 32.0),
            "Stepper" => (94.0, 32.0),
            "SearchBar" => (320.0, 56.0),
            // El desplegable, con el nombre que usa el núcleo. La etiqueta se
            // llama `<an-select>` desde que las etiquetas llevan prefijo, pero
            // lo que llega aquí es `NodeKind::control_name()`, y eso sigue
            // diciendo `Picker`. Mientras puso "Select" no lo reconocía nadie
            // y el desplegable medía cero: sin error, sin traza, y visible
            // solo si la plantilla no le daba un alto explícito.
            "Picker" => (140.0, 44.0),
            "DatePicker" => (200.0, 44.0),
            "NavigationBar" => (320.0, 44.0),
            _ => (0.0, 0.0),
        }
    }
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

#[cfg(test)]
mod min_content_tests {
    use super::*;

    /// El mínimo de un texto es su palabra más larga, no cero.
    ///
    /// Este es el caso que hacía desaparecer texto en el dispositivo: un
    /// `<Text>` dentro de un contenedor centrado no tiene ancho impuesto, así
    /// que el layout se queda con el mínimo, y con el mínimo a cero el texto
    /// se quedaba en una caja de ancho cero.
    #[test]
    fn el_minimo_es_la_palabra_mas_larga() {
        let font = FontSpec { size: 10.0, ..Default::default() };
        let (width, height) = NaiveMeasurer.measure_text_min_content("hola mundo enorme", &font);
        let (solo, _) = NaiveMeasurer.measure_text("enorme", &font, None);
        assert_eq!(width, solo);
        assert!(width > 0.0);
        // Partido a ese ancho caben tres líneas: el alto no es el de una.
        let (_, una_linea) = NaiveMeasurer.measure_text("enorme", &font, None);
        assert!(height > una_linea);
    }

    /// Una sola palabra no se parte: su mínimo es ella entera.
    #[test]
    fn una_palabra_suelta_no_se_parte() {
        let font = FontSpec { size: 10.0, ..Default::default() };
        let entera = NaiveMeasurer.measure_text("indivisible", &font, None);
        assert_eq!(NaiveMeasurer.measure_text_min_content("indivisible", &font), entera);
    }
}
