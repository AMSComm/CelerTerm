use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent};

pub struct TextRenderer {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub cell_width: f32,
    pub cell_height: f32,
    pub ligatures: bool,
}

impl TextRenderer {
    pub fn new(family: &str, size: f32, line_height_factor: f32) -> Self {
        Self::with_fallbacks(family, &[], size, line_height_factor)
    }

    pub fn with_fallbacks(family: &str, fallbacks: &[String], size: f32, line_height_factor: f32) -> Self {
        Self::with_options(family, fallbacks, size, line_height_factor, true)
    }

    pub fn with_options(
        family: &str,
        fallbacks: &[String],
        size: f32,
        line_height_factor: f32,
        ligatures: bool,
    ) -> Self {
        let mut font_system = FontSystem::new();

        if let Some(base_dirs) = directories::BaseDirs::new() {
            let user_fonts = base_dirs.home_dir().join("Library").join("Fonts");
            if user_fonts.exists() {
                font_system.db_mut().load_fonts_dir(&user_fonts);
            }
            let linux_user_fonts = base_dirs.home_dir().join(".local").join("share").join("fonts");
            if linux_user_fonts.exists() {
                font_system.db_mut().load_fonts_dir(&linux_user_fonts);
            }
        }

        let effective_family = if font_system.db().faces().any(|f| f.families.iter().any(|(fam, _)| fam == family)) {
            family.to_string()
        } else {
            let mut found = None;
            for fb in fallbacks {
                if font_system.db().faces().any(|f| f.families.iter().any(|(fam, _)| fam == fb)) {
                    found = Some(fb.clone());
                    break;
                }
            }
            found.unwrap_or_else(|| family.to_string())
        };

        let swash_cache = SwashCache::new();
        let line_height = (size * line_height_factor).round();

        // Calculate monospace cell width by measuring a sample character 'M'
        let metrics = Metrics::new(size, line_height);
        let mut buffer = Buffer::new(&mut font_system, metrics);
        let attrs = Attrs::new().family(Family::Name(&effective_family));
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
            font_family: effective_family,
            font_size: size,
            line_height,
            cell_width,
            cell_height: line_height,
            ligatures,
        }
    }

    pub fn shape_line(&mut self, text: &str) -> usize {
        let metrics = Metrics::new(self.font_size, self.line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let attrs = Attrs::new().family(Family::Name(&self.font_family));

        let shaping = if self.ligatures { Shaping::Advanced } else { Shaping::Basic };
        buffer.set_text(&mut self.font_system, text, attrs, shaping);
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

        let shaping = if self.ligatures { Shaping::Advanced } else { Shaping::Basic };
        buffer.set_text(&mut self.font_system, text, attrs, shaping);
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

    pub fn is_box_or_block(c: char) -> bool {
        ('\u{2500}'..='\u{257f}').contains(&c) || ('\u{2580}'..='\u{259f}').contains(&c)
    }

    pub fn is_nerd_font_or_pua(c: char) -> bool {
        ('\u{e000}'..='\u{f8ff}').contains(&c)
            || ('\u{f0000}'..='\u{ffffd}').contains(&c)
            || ('\u{100000}'..='\u{10fffd}').contains(&c)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_box_or_block_char(
        &self,
        target: &mut [u32],
        target_width: usize,
        target_height: usize,
        cell_x: f32,
        cell_y: f32,
        c: char,
        color: u32,
    ) {
        let x0 = cell_x.round() as usize;
        let y0 = cell_y.round() as usize;
        let x1 = (cell_x + self.cell_width).round() as usize;
        let y1 = (cell_y + self.cell_height).round() as usize;
        let w = x1.saturating_sub(x0);
        let h = y1.saturating_sub(y0);
        let mid_x = x0 + w / 2;
        let mid_y = y0 + h / 2;

        // Block elements
        if ('\u{2580}'..='\u{259f}').contains(&c) {
            match c {
                '█' => Self::draw_rect(target, target_width, target_height, x0, y0, w, h, color),
                '▀' => Self::draw_rect(target, target_width, target_height, x0, y0, w, h / 2, color),
                '▄' => Self::draw_rect(target, target_width, target_height, x0, mid_y, w, y1.saturating_sub(mid_y), color),
                '▌' => Self::draw_rect(target, target_width, target_height, x0, y0, w / 2, h, color),
                '▐' => Self::draw_rect(target, target_width, target_height, mid_x, y0, x1.saturating_sub(mid_x), h, color),
                ' '..='▇' => {
                    let frac = (c as u32 - 0x2580) as usize;
                    let bar_h = (h * frac) / 8;
                    Self::draw_rect(target, target_width, target_height, x0, y1.saturating_sub(bar_h), w, bar_h, color);
                }
                '▔' => {
                    let bar_h = (h / 8).max(1);
                    Self::draw_rect(target, target_width, target_height, x0, y0, w, bar_h, color);
                }
                '▕' => {
                    let bar_w = (w / 8).max(1);
                    Self::draw_rect(target, target_width, target_height, x1.saturating_sub(bar_w), y0, bar_w, h, color);
                }
                '▖' => Self::draw_rect(target, target_width, target_height, x0, mid_y, w / 2, y1.saturating_sub(mid_y), color),
                '▗' => Self::draw_rect(target, target_width, target_height, mid_x, mid_y, x1.saturating_sub(mid_x), y1.saturating_sub(mid_y), color),
                '▘' => Self::draw_rect(target, target_width, target_height, x0, y0, w / 2, mid_y.saturating_sub(y0), color),
                '▝' => Self::draw_rect(target, target_width, target_height, mid_x, y0, x1.saturating_sub(mid_x), mid_y.saturating_sub(y0), color),
                '▚' => {
                    Self::draw_rect(target, target_width, target_height, x0, y0, w / 2, mid_y.saturating_sub(y0), color);
                    Self::draw_rect(target, target_width, target_height, mid_x, mid_y, x1.saturating_sub(mid_x), y1.saturating_sub(mid_y), color);
                }
                '▞' => {
                    Self::draw_rect(target, target_width, target_height, mid_x, y0, x1.saturating_sub(mid_x), mid_y.saturating_sub(y0), color);
                    Self::draw_rect(target, target_width, target_height, x0, mid_y, w / 2, y1.saturating_sub(mid_y), color);
                }
                '░' | '▒' | '▓' => {
                    let step = if c == '▒' { 2 } else if c == '░' { 3 } else { 2 };
                    for py in y0..y1.min(target_height) {
                        for px in x0..x1.min(target_width) {
                            let hit = match c {
                                '▒' => (px + py) % 2 == 0,
                                '░' => px % step == 0 && py % step == 0,
                                '▓' => (px + py) % 2 == 0 || (px % 2 == 0),
                                _ => false,
                            };
                            if hit {
                                target[py * target_width + px] = color;
                            }
                        }
                    }
                }
                _ => {
                    Self::draw_rect(target, target_width, target_height, x0, y0, w, h, color);
                }
            }
            return;
        }

        // Box drawing lines: (up, down, left, right)
        // 0: none, 1: light, 2: heavy, 3: double
        let arms = match c {
            '─' | '┄' | '┈' | '╌' => (0, 0, 1, 1),
            '━' | '┅' | '┉' | '╍' => (0, 0, 2, 2),
            '═' => (0, 0, 3, 3),
            '│' | '┆' | '┊' | '╎' => (1, 1, 0, 0),
            '┃' | '┇' | '︙' | '╏' => (2, 2, 0, 0),
            '║' => (3, 3, 0, 0),
            '┌' | '╭' => (0, 1, 0, 1),
            '┏' => (0, 2, 0, 2),
            '╔' => (0, 3, 0, 3),
            '┐' | '╮' => (0, 1, 1, 0),
            '┓' => (0, 2, 2, 0),
            '╗' => (0, 3, 3, 0),
            '└' | '╰' => (1, 0, 0, 1),
            '┗' => (2, 0, 0, 2),
            '╚' => (3, 0, 0, 3),
            '┘' | '╯' => (1, 0, 1, 0),
            '┛' => (2, 0, 2, 0),
            '╝' => (3, 0, 3, 0),
            '├' => (1, 1, 0, 1),
            '┣' => (2, 2, 0, 2),
            '╠' => (3, 3, 0, 3),
            '╡' | '╟' => (1, 1, 0, 3),
            '┤' => (1, 1, 1, 0),
            '┫' => (2, 2, 2, 0),
            '╣' => (3, 3, 3, 0),
            '┬' => (0, 1, 1, 1),
            '┳' => (0, 2, 2, 2),
            '╦' => (0, 3, 3, 3),
            '┴' => (1, 0, 1, 1),
            '┻' => (2, 0, 2, 2),
            '╩' => (3, 0, 3, 3),
            '┼' => (1, 1, 1, 1),
            '╋' => (2, 2, 2, 2),
            '╬' => (3, 3, 3, 3),
            '╴' => (0, 0, 1, 0),
            '╵' => (1, 0, 0, 0),
            '╶' => (0, 0, 0, 1),
            '╷' => (0, 1, 0, 0),
            '╸' => (0, 0, 2, 0),
            '╹' => (2, 0, 0, 0),
            '╺' => (0, 0, 0, 2),
            '╻' => (0, 2, 0, 0),
            _ => (1, 1, 1, 1),
        };

        let (up, down, left, right) = arms;

        // Draw UP arm
        match up {
            1 => Self::draw_rect(target, target_width, target_height, mid_x, y0, 1, (mid_y + 1).saturating_sub(y0), color),
            2 => Self::draw_rect(target, target_width, target_height, mid_x.saturating_sub(1), y0, 2, (mid_y + 1).saturating_sub(y0), color),
            3 => {
                Self::draw_rect(target, target_width, target_height, mid_x.saturating_sub(2), y0, 1, (mid_y + 2).saturating_sub(y0), color);
                Self::draw_rect(target, target_width, target_height, mid_x + 1, y0, 1, (mid_y + 2).saturating_sub(y0), color);
            }
            _ => {}
        }

        // Draw DOWN arm
        match down {
            1 => Self::draw_rect(target, target_width, target_height, mid_x, mid_y, 1, y1.saturating_sub(mid_y), color),
            2 => Self::draw_rect(target, target_width, target_height, mid_x.saturating_sub(1), mid_y, 2, y1.saturating_sub(mid_y), color),
            3 => {
                let sy = mid_y.saturating_sub(1);
                Self::draw_rect(target, target_width, target_height, mid_x.saturating_sub(2), sy, 1, y1.saturating_sub(sy), color);
                Self::draw_rect(target, target_width, target_height, mid_x + 1, sy, 1, y1.saturating_sub(sy), color);
            }
            _ => {}
        }

        // Draw LEFT arm
        match left {
            1 => Self::draw_rect(target, target_width, target_height, x0, mid_y, (mid_x + 1).saturating_sub(x0), 1, color),
            2 => Self::draw_rect(target, target_width, target_height, x0, mid_y.saturating_sub(1), (mid_x + 1).saturating_sub(x0), 2, color),
            3 => {
                Self::draw_rect(target, target_width, target_height, x0, mid_y.saturating_sub(2), (mid_x + 2).saturating_sub(x0), 1, color);
                Self::draw_rect(target, target_width, target_height, x0, mid_y + 1, (mid_x + 2).saturating_sub(x0), 1, color);
            }
            _ => {}
        }

        // Draw RIGHT arm
        match right {
            1 => Self::draw_rect(target, target_width, target_height, mid_x, mid_y, x1.saturating_sub(mid_x), 1, color),
            2 => Self::draw_rect(target, target_width, target_height, mid_x, mid_y.saturating_sub(1), x1.saturating_sub(mid_x), 2, color),
            3 => {
                let sx = mid_x.saturating_sub(1);
                Self::draw_rect(target, target_width, target_height, sx, mid_y.saturating_sub(2), x1.saturating_sub(sx), 1, color);
                Self::draw_rect(target, target_width, target_height, sx, mid_y + 1, x1.saturating_sub(sx), 1, color);
            }
            _ => {}
        }
    }
}
