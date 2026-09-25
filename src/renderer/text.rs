use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent};

pub struct TextRenderer {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub cell_width: f32,
    pub cell_height: f32,
}

impl TextRenderer {
    pub fn new(family: &str, size: f32, line_height_factor: f32) -> Self {
        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let line_height = (size * line_height_factor).round();

        // Calculate monospace cell width by measuring a sample character 'M'
        let metrics = Metrics::new(size, line_height);
        let mut buffer = Buffer::new(&mut font_system, metrics);
        let attrs = Attrs::new().family(Family::Name(family));
        buffer.set_text(&mut font_system, "M", attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut font_system, false);

        let mut cell_width = (size * 0.6).round().max(7.0);
        for run in buffer.layout_runs() {
            if let Some(glyph) = run.glyphs.first()
                && glyph.w > 0.0
            {
                cell_width = glyph.w.round().max(7.0);
            }
        }

        Self {
            font_system,
            swash_cache,
            font_family: family.to_string(),
            font_size: size,
            line_height,
            cell_width,
            cell_height: line_height,
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

    #[allow(clippy::too_many_arguments)]
    pub fn draw_text(
        &mut self,
        target: &mut [u32],
        target_width: usize,
        target_height: usize,
        start_x: f32,
        start_y: f32,
        text: &str,
        color: u32, // 0x00RRGGBB
    ) {
        if text.is_empty() {
            return;
        }

        let metrics = Metrics::new(self.font_size, self.line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let attrs = Attrs::new().family(Family::Name(&self.font_family));

        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let r = (color >> 16) & 0xFF;
        let g = (color >> 8) & 0xFF;
        let b = color & 0xFF;

        for run in buffer.layout_runs() {
            let line_y = start_y + run.line_y;

            for glyph in run.glyphs {
                let phys = glyph.physical((start_x, line_y), 1.0);
                if let Some(image) = self.swash_cache.get_image(&mut self.font_system, phys.cache_key) {
                    let gx = phys.x + image.placement.left;
                    let gy = phys.y - image.placement.top;

                    let img_w = image.placement.width as usize;
                    let img_h = image.placement.height as usize;

                    match image.content {
                        SwashContent::Mask => {
                            for iy in 0..img_h {
                                let py = gy + iy as i32;
                                if py < 0 || (py as usize) >= target_height {
                                    continue;
                                }

                                for ix in 0..img_w {
                                    let px = gx + ix as i32;
                                    if px < 0 || (px as usize) >= target_width {
                                        continue;
                                    }

                                    let alpha = image.data[iy * img_w + ix] as u32;
                                    if alpha == 0 {
                                        continue;
                                    }

                                    let target_idx = (py as usize) * target_width + (px as usize);
                                    if alpha >= 255 {
                                        target[target_idx] = (r << 16) | (g << 8) | b;
                                    } else {
                                        // Alpha blend with background
                                        let bg = target[target_idx];
                                        let bg_r = (bg >> 16) & 0xFF;
                                        let bg_g = (bg >> 8) & 0xFF;
                                        let bg_b = bg & 0xFF;

                                        let out_r = (r * alpha + bg_r * (255 - alpha)) / 255;
                                        let out_g = (g * alpha + bg_g * (255 - alpha)) / 255;
                                        let out_b = (b * alpha + bg_b * (255 - alpha)) / 255;

                                        target[target_idx] = (out_r << 16) | (out_g << 8) | out_b;
                                    }
                                }
                            }
                        }
                        SwashContent::Color => {
                            for iy in 0..img_h {
                                let py = gy + iy as i32;
                                if py < 0 || (py as usize) >= target_height {
                                    continue;
                                }

                                for ix in 0..img_w {
                                    let px = gx + ix as i32;
                                    if px < 0 || (px as usize) >= target_width {
                                        continue;
                                    }

                                    let offset = (iy * img_w + ix) * 4;
                                    let cr = image.data[offset] as u32;
                                    let cg = image.data[offset + 1] as u32;
                                    let cb = image.data[offset + 2] as u32;
                                    let ca = image.data[offset + 3] as u32;

                                    if ca == 0 {
                                        continue;
                                    }

                                    let target_idx = (py as usize) * target_width + (px as usize);
                                    if ca >= 255 {
                                        target[target_idx] = (cr << 16) | (cg << 8) | cb;
                                    } else {
                                        let bg = target[target_idx];
                                        let bg_r = (bg >> 16) & 0xFF;
                                        let bg_g = (bg >> 8) & 0xFF;
                                        let bg_b = bg & 0xFF;

                                        let out_r = (cr * ca + bg_r * (255 - ca)) / 255;
                                        let out_g = (cg * ca + bg_g * (255 - ca)) / 255;
                                        let out_b = (cb * ca + bg_b * (255 - ca)) / 255;

                                        target[target_idx] = (out_r << 16) | (out_g << 8) | out_b;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_rect(
        target: &mut [u32],
        target_width: usize,
        target_height: usize,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        color: u32,
    ) {
        let max_x = (x + width).min(target_width);
        let max_y = (y + height).min(target_height);

        for py in y..max_y {
            let row_offset = py * target_width;
            for px in x..max_x {
                target[row_offset + px] = color;
            }
        }
    }
}
