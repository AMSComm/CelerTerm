use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};

pub struct TextRenderer {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
}

impl TextRenderer {
    pub fn new(family: &str, size: f32, line_height_factor: f32) -> Self {
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let line_height = size * line_height_factor;

        Self {
            font_system,
            swash_cache,
            font_family: family.to_string(),
            font_size: size,
            line_height,
        }
    }

    pub fn shape_line(&mut self, text: &str) -> usize {
        let metrics = Metrics::new(self.font_size, self.line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let attrs = Attrs::new().family(Family::Name(&self.font_family));

        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut glyph_count = 0;
        for run in buffer.layout_runs() {
            glyph_count += run.glyphs.len();
        }
        glyph_count
    }
}
