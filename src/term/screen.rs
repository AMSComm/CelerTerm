use std::fmt::Write as FmtWrite;
use std::io::Write as IoWrite;
use std::sync::Arc;
use parking_lot::Mutex;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, GridCell, Row};
use alacritty_terminal::index::{Boundary, Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionRange, SelectionType};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor, Rgb};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermSize {
    pub columns: usize,
    pub lines: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize { self.lines }
    fn screen_lines(&self) -> usize { self.lines }
    fn columns(&self) -> usize { self.columns }
}

pub type SharedPtyWriter = Arc<Mutex<Box<dyn IoWrite + Send>>>;

#[derive(Clone, Default)]
pub struct PtySink {
    inner: Arc<Mutex<Option<SharedPtyWriter>>>,
    title: Arc<Mutex<Option<String>>>,
}

impl PtySink {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            title: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_writer(&self, writer: SharedPtyWriter) {
        *self.inner.lock() = Some(writer);
    }

    pub fn dynamic_title(&self) -> Option<String> {
        self.title.lock().clone()
    }

    pub fn reset_dynamic_title(&self) {
        *self.title.lock() = None;
    }
}

impl EventListener for PtySink {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => {
                if let Some(writer) = self.inner.lock().as_ref() {
                    let mut w = writer.lock();
                    let _ = w.write_all(text.as_bytes());
                    let _ = w.flush();
                }
            }
            Event::Title(t) => {
                *self.title.lock() = Some(t);
            }
            Event::ResetTitle => {
                *self.title.lock() = None;
            }
            _ => {}
        }
    }
}

pub struct TermScreen {
    pub term: Term<PtySink>,
    processor: Processor,
    pub size: TermSize,
    pub dirty: bool,
    pub sink: PtySink,
    pub selection_anchor: Option<Point>,
    pub selection_head: Option<Point>,
}

impl TermScreen {
    pub fn new(columns: usize, lines: usize) -> Self {
        let size = TermSize { columns, lines };
        let sink = PtySink::new();
        let term = Term::new(Config::default(), &size, sink.clone());
        let processor = Processor::default();

        Self {
            term,
            processor,
            size,
            dirty: true,
            sink,
            selection_anchor: None,
            selection_head: None,
        }
    }

    pub fn set_pty_writer(&self, writer: Arc<Mutex<Box<dyn IoWrite + Send>>>) {
        self.sink.set_writer(writer);
    }

    pub fn dynamic_title(&self) -> Option<String> {
        self.sink.dynamic_title()
    }

    pub fn reset_dynamic_title(&self) {
        self.sink.reset_dynamic_title();
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.processor.advance(&mut self.term, byte);
        }
        self.dirty = true;
    }

    pub fn resize(&mut self, columns: usize, lines: usize) {
        self.size = TermSize { columns, lines };
        self.term.resize(self.size);
        self.dirty = true;
    }

    pub fn scroll_display(&mut self, delta: i32) {
        self.term.scroll_display(alacritty_terminal::grid::Scroll::Delta(delta));
        self.dirty = true;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(alacritty_terminal::grid::Scroll::Bottom);
        self.dirty = true;
    }

    pub fn scroll_to_top(&mut self) {
        self.term.scroll_display(alacritty_terminal::grid::Scroll::Top);
        self.dirty = true;
    }

    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    pub fn history_size(&self) -> usize {
        let grid = self.term.grid();
        grid.total_lines().saturating_sub(grid.screen_lines())
    }

    pub fn scroll_to_offset(&mut self, target_offset: usize) {
        let max_offset = self.history_size();
        let clamped = target_offset.min(max_offset);
        let current = self.display_offset();
        let delta = clamped as i32 - current as i32;
        if delta != 0 {
            self.scroll_display(delta);
        }
    }

    pub fn is_alt_screen(&self) -> bool {
        self.term.mode().contains(alacritty_terminal::term::TermMode::ALT_SCREEN)
    }

    pub fn is_app_cursor(&self) -> bool {
        self.term.mode().contains(alacritty_terminal::term::TermMode::APP_CURSOR)
    }

    pub fn is_app_keypad(&self) -> bool {
        self.term.mode().contains(alacritty_terminal::term::TermMode::APP_KEYPAD)
    }

    pub fn is_bracketed_paste(&self) -> bool {
        self.term.mode().contains(alacritty_terminal::term::TermMode::BRACKETED_PASTE)
    }

    pub fn is_mouse_mode(&self) -> bool {
        self.term.mode().intersects(
            alacritty_terminal::term::TermMode::MOUSE_REPORT_CLICK
                | alacritty_terminal::term::TermMode::MOUSE_DRAG
                | alacritty_terminal::term::TermMode::MOUSE_MOTION
                | alacritty_terminal::term::TermMode::SGR_MOUSE,
        )
    }

    pub fn is_sgr_mouse(&self) -> bool {
        self.term.mode().contains(alacritty_terminal::term::TermMode::SGR_MOUSE)
    }

    pub fn is_mouse_drag(&self) -> bool {
        self.term.mode().intersects(
            alacritty_terminal::term::TermMode::MOUSE_DRAG
                | alacritty_terminal::term::TermMode::MOUSE_MOTION,
        )
    }

    pub fn is_mouse_motion(&self) -> bool {
        self.term.mode().contains(alacritty_terminal::term::TermMode::MOUSE_MOTION)
    }

    pub fn is_focus_reporting(&self) -> bool {
        self.term.mode().contains(alacritty_terminal::term::TermMode::FOCUS_IN_OUT)
    }

    pub fn is_show_cursor(&self) -> bool {
        self.term.mode().contains(alacritty_terminal::term::TermMode::SHOW_CURSOR)
    }

    pub fn cursor_shape(&self) -> alacritty_terminal::vte::ansi::CursorShape {
        if !self.term.mode().contains(alacritty_terminal::term::TermMode::SHOW_CURSOR) {
            alacritty_terminal::vte::ansi::CursorShape::Hidden
        } else {
            self.term.cursor_style().shape
        }
    }

    pub fn is_wide_cell(&self, col: usize, line: usize) -> bool {
        let grid = self.term.grid();
        let display_line = Line(line as i32 - grid.display_offset() as i32);
        if display_line < Line(0) || display_line.0 >= self.size.lines as i32 || col >= self.size.columns {
            return false;
        }
        let cell = &grid[display_line][Column(col)];
        cell.flags.contains(Flags::WIDE_CHAR)
            || (cell.c != ' ' && cell.c != '\0' && unicode_width::UnicodeWidthChar::width(cell.c) == Some(2))
    }

    pub fn start_selection(&mut self, ty: SelectionType, point: Point, side: Side) {
        self.selection_anchor = Some(point);
        self.selection_head = Some(point);
        self.term.selection = Some(Selection::new(ty, point, side));
        self.dirty = true;
    }

    pub fn update_selection(&mut self, point: Point, side: Side) {
        self.selection_head = Some(point);
        if let Some(ref mut sel) = self.term.selection {
            sel.update(point, side);
            self.dirty = true;
        }
    }

    pub fn clear_selection(&mut self) {
        if self.term.selection.is_some() {
            self.term.selection = None;
            self.dirty = true;
        }
    }

    pub fn clear_selection_and_anchor(&mut self) {
        self.selection_anchor = None;
        self.selection_head = None;
        self.clear_selection();
    }

    pub fn extend_selection_to(&mut self, target_point: Point, target_side: Side) {
        let anchor = if let Some(anchor) = self.selection_anchor {
            anchor
        } else if let Some(range) = self.selection_range() {
            let cols = self.size.columns.max(1);
            let idx_target = target_point.line.0 as i64 * cols as i64 + target_point.column.0 as i64;
            let idx_start = range.start.line.0 as i64 * cols as i64 + range.start.column.0 as i64;
            let idx_end = range.end.line.0 as i64 * cols as i64 + range.end.column.0 as i64;
            if (idx_target - idx_start).abs() < (idx_target - idx_end).abs() {
                range.end
            } else {
                range.start
            }
        } else {
            self.term.grid().cursor.point
        };

        self.selection_anchor = Some(anchor);
        self.selection_head = Some(target_point);

        let ty = self.term.selection.as_ref().map(|s| s.ty).unwrap_or(SelectionType::Simple);

        let (anchor_side, head_side) = if anchor <= target_point {
            (Side::Left, target_side)
        } else {
            (Side::Right, target_side)
        };

        let mut sel = Selection::new(ty, anchor, anchor_side);
        sel.update(target_point, head_side);
        if anchor != target_point {
            sel.include_all();
        }
        self.term.selection = Some(sel);
        self.dirty = true;
    }

    pub fn select_right(&mut self) {
        let is_active = self.term.selection.is_some() && self.selection_range().is_some();
        let anchor = self.selection_anchor.unwrap_or_else(|| {
            let cur = self.term.grid().cursor.point;
            self.selection_anchor = Some(cur);
            cur
        });

        if !is_active {
            self.selection_head = Some(anchor);
            let mut sel = Selection::new(SelectionType::Simple, anchor, Side::Left);
            sel.update(anchor, Side::Right);
            sel.include_all();
            self.term.selection = Some(sel);
            self.dirty = true;
        } else {
            let head = self.selection_head.unwrap_or(anchor);
            if head < anchor {
                let next_head = head.add(&self.term, Boundary::Grid, 1);
                if next_head == anchor {
                    self.clear_selection();
                    self.selection_head = None;
                } else {
                    self.selection_head = Some(next_head);
                    let mut sel = Selection::new(
                        SelectionType::Simple,
                        anchor.sub(&self.term, Boundary::Grid, 1),
                        Side::Right,
                    );
                    sel.update(next_head, Side::Left);
                    sel.include_all();
                    self.term.selection = Some(sel);
                    self.dirty = true;
                }
            } else {
                let next_head = head.add(&self.term, Boundary::Grid, 1);
                self.selection_head = Some(next_head);
                let mut sel = Selection::new(SelectionType::Simple, anchor, Side::Left);
                sel.update(next_head, Side::Right);
                sel.include_all();
                self.term.selection = Some(sel);
                self.dirty = true;
            }
        }
    }

    pub fn select_left(&mut self) {
        let is_active = self.term.selection.is_some() && self.selection_range().is_some();
        let anchor = self.selection_anchor.unwrap_or_else(|| {
            let cur = self.term.grid().cursor.point;
            self.selection_anchor = Some(cur);
            cur
        });

        if !is_active {
            let head = anchor.sub(&self.term, Boundary::Grid, 1);
            if head == anchor {
                return;
            }
            self.selection_head = Some(head);
            let mut sel = Selection::new(
                SelectionType::Simple,
                anchor.sub(&self.term, Boundary::Grid, 1),
                Side::Right,
            );
            sel.update(head, Side::Left);
            sel.include_all();
            self.term.selection = Some(sel);
            self.dirty = true;
        } else {
            let head = self.selection_head.unwrap_or(anchor);
            if head > anchor {
                let next_head = head.sub(&self.term, Boundary::Grid, 1);
                if next_head == anchor {
                    self.selection_head = Some(anchor);
                    let mut sel = Selection::new(SelectionType::Simple, anchor, Side::Left);
                    sel.update(anchor, Side::Right);
                    sel.include_all();
                    self.term.selection = Some(sel);
                    self.dirty = true;
                } else if next_head < anchor {
                    self.clear_selection();
                    self.selection_head = None;
                } else {
                    self.selection_head = Some(next_head);
                    let mut sel = Selection::new(SelectionType::Simple, anchor, Side::Left);
                    sel.update(next_head, Side::Right);
                    sel.include_all();
                    self.term.selection = Some(sel);
                    self.dirty = true;
                }
            } else if head == anchor {
                self.clear_selection();
                self.selection_head = None;
            } else {
                let next_head = head.sub(&self.term, Boundary::Grid, 1);
                self.selection_head = Some(next_head);
                let mut sel = Selection::new(
                    SelectionType::Simple,
                    anchor.sub(&self.term, Boundary::Grid, 1),
                    Side::Right,
                );
                sel.update(next_head, Side::Left);
                sel.include_all();
                self.term.selection = Some(sel);
                self.dirty = true;
            }
        }
    }

    pub fn select_down(&mut self) {
        let is_active = self.term.selection.is_some() && self.selection_range().is_some();
        let anchor = self.selection_anchor.unwrap_or_else(|| {
            let cur = self.term.grid().cursor.point;
            self.selection_anchor = Some(cur);
            cur
        });

        let head = if is_active {
            self.selection_head.unwrap_or(anchor)
        } else {
            anchor
        };

        let next_head = Point::new(head.line + 1, head.column).grid_clamp(&self.term, Boundary::Grid);
        if next_head == head {
            return;
        }

        self.selection_head = Some(next_head);
        let (anchor_side, head_side) = if anchor <= next_head {
            (Side::Left, Side::Right)
        } else {
            (Side::Right, Side::Left)
        };
        let mut sel = Selection::new(SelectionType::Simple, anchor, anchor_side);
        sel.update(next_head, head_side);
        sel.include_all();
        self.term.selection = Some(sel);
        self.dirty = true;
    }

    pub fn select_up(&mut self) {
        let is_active = self.term.selection.is_some() && self.selection_range().is_some();
        let anchor = self.selection_anchor.unwrap_or_else(|| {
            let cur = self.term.grid().cursor.point;
            self.selection_anchor = Some(cur);
            cur
        });

        let head = if is_active {
            self.selection_head.unwrap_or(anchor)
        } else {
            anchor
        };

        let next_head = Point::new(head.line - 1, head.column).grid_clamp(&self.term, Boundary::Grid);
        if next_head == head {
            return;
        }

        self.selection_head = Some(next_head);
        let (anchor_side, head_side) = if anchor <= next_head {
            (Side::Left, Side::Right)
        } else {
            (Side::Right, Side::Left)
        };
        let mut sel = Selection::new(SelectionType::Simple, anchor, anchor_side);
        sel.update(next_head, head_side);
        sel.include_all();
        self.term.selection = Some(sel);
        self.dirty = true;
    }

    pub fn selection_range(&self) -> Option<SelectionRange> {
        self.term.selection.as_ref().and_then(|s| s.to_range(&self.term))
    }

    pub fn copy_selection_text(&self) -> Option<String> {
        self.term.selection_to_string()
    }

    pub fn is_point_selected(&self, col: usize, line: usize) -> bool {
        if let Some(range) = self.selection_range() {
            let grid = self.term.grid();
            let display_line = Line(line as i32 - grid.display_offset() as i32);
            range.contains(Point::new(display_line, Column(col)))
        } else {
            false
        }
    }

    pub fn get_cell_char(&self, col: usize, line: usize) -> char {
        let grid = self.term.grid();
        let display_line = Line(line as i32 - grid.display_offset() as i32);
        grid[display_line][Column(col)].c
    }

    pub fn get_line_string(&self, line: usize) -> String {
        let grid = self.term.grid();
        let display_line = Line(line as i32 - grid.display_offset() as i32);
        let mut s = String::new();
        for col in 0..self.size.columns {
            s.push(grid[display_line][Column(col)].c);
        }
        s.trim_end().to_string()
    }

    pub fn get_screen_text(&self) -> Vec<String> {
        (0..self.size.lines)
            .map(|l| self.get_line_string(l))
            .filter(|line| !line.is_empty())
            .collect()
    }

    pub fn get_scrollback_lines(&self, max_lines: usize) -> Vec<String> {
        let grid = self.term.grid();
        let total_lines = grid.total_lines();
        let visible_lines = grid.screen_lines();
        let history_lines = total_lines.saturating_sub(visible_lines);

        let mut lines = Vec::new();

        let hist_to_take = history_lines.min(max_lines);
        let start_h = history_lines - hist_to_take;

        for h in start_h..history_lines {
            let offset = (history_lines - h) as i32;
            let line_idx = Line(-offset);
            lines.push(Self::serialize_row_to_ansi(&grid[line_idx], self.size.columns));
        }

        for v in 0..visible_lines {
            let line_idx = Line(v as i32);
            lines.push(Self::serialize_row_to_ansi(&grid[line_idx], self.size.columns));
        }

        while let Some(last) = lines.last() {
            if last.is_empty() {
                lines.pop();
            } else {
                break;
            }
        }

        lines
    }

    fn serialize_row_to_ansi(row: &Row<Cell>, cols: usize) -> String {
        let last_col = match (0..cols).rposition(|c| !row[Column(c)].is_empty()) {
            Some(idx) => idx,
            None => return String::new(),
        };

        #[derive(Clone, Copy, PartialEq, Eq)]
        struct StyleState {
            fg: Color,
            bg: Color,
            bold: bool,
            dim: bool,
            italic: bool,
            underline: bool,
            double_underline: bool,
            inverse: bool,
            strikeout: bool,
        }

        impl Default for StyleState {
            fn default() -> Self {
                Self {
                    fg: Color::Named(NamedColor::Foreground),
                    bg: Color::Named(NamedColor::Background),
                    bold: false,
                    dim: false,
                    italic: false,
                    underline: false,
                    double_underline: false,
                    inverse: false,
                    strikeout: false,
                }
            }
        }

        let mut current_style = StyleState::default();
        let mut out = String::new();

        for c_idx in 0..=last_col {
            let cell = &row[Column(c_idx)];

            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }

            let cell_style = StyleState {
                fg: cell.fg,
                bg: cell.bg,
                bold: cell.flags.contains(Flags::BOLD),
                dim: cell.flags.contains(Flags::DIM),
                italic: cell.flags.contains(Flags::ITALIC),
                underline: cell.flags.contains(Flags::UNDERLINE),
                double_underline: cell.flags.contains(Flags::DOUBLE_UNDERLINE),
                inverse: cell.flags.contains(Flags::INVERSE),
                strikeout: cell.flags.contains(Flags::STRIKEOUT),
            };

            if cell_style != current_style {
                let mut sgr = String::from("\x1b[0");

                if cell_style.bold {
                    sgr.push_str(";1");
                }
                if cell_style.dim {
                    sgr.push_str(";2");
                }
                if cell_style.italic {
                    sgr.push_str(";3");
                }
                if cell_style.underline {
                    sgr.push_str(";4");
                }
                if cell_style.double_underline {
                    sgr.push_str(";21");
                }
                if cell_style.inverse {
                    sgr.push_str(";7");
                }
                if cell_style.strikeout {
                    sgr.push_str(";9");
                }

                match cell_style.fg {
                    Color::Named(named) => match named {
                        NamedColor::Black => sgr.push_str(";30"),
                        NamedColor::Red => sgr.push_str(";31"),
                        NamedColor::Green => sgr.push_str(";32"),
                        NamedColor::Yellow => sgr.push_str(";33"),
                        NamedColor::Blue => sgr.push_str(";34"),
                        NamedColor::Magenta => sgr.push_str(";35"),
                        NamedColor::Cyan => sgr.push_str(";36"),
                        NamedColor::White => sgr.push_str(";37"),
                        NamedColor::BrightBlack => sgr.push_str(";90"),
                        NamedColor::BrightRed => sgr.push_str(";91"),
                        NamedColor::BrightGreen => sgr.push_str(";92"),
                        NamedColor::BrightYellow => sgr.push_str(";93"),
                        NamedColor::BrightBlue => sgr.push_str(";94"),
                        NamedColor::BrightMagenta => sgr.push_str(";95"),
                        NamedColor::BrightCyan => sgr.push_str(";96"),
                        NamedColor::BrightWhite => sgr.push_str(";97"),
                        NamedColor::Foreground => {},
                        NamedColor::BrightForeground => sgr.push_str(";97"),
                        NamedColor::DimBlack => sgr.push_str(";2;30"),
                        NamedColor::DimRed => sgr.push_str(";2;31"),
                        NamedColor::DimGreen => sgr.push_str(";2;32"),
                        NamedColor::DimYellow => sgr.push_str(";2;33"),
                        NamedColor::DimBlue => sgr.push_str(";2;34"),
                        NamedColor::DimMagenta => sgr.push_str(";2;35"),
                        NamedColor::DimCyan => sgr.push_str(";2;36"),
                        NamedColor::DimWhite => sgr.push_str(";2;37"),
                        NamedColor::DimForeground => sgr.push_str(";2;39"),
                        _ => {},
                    },
                    Color::Indexed(idx) => {
                        let _ = write!(sgr, ";38;5;{}", idx);
                    }
                    Color::Spec(rgb) => {
                        let _ = write!(sgr, ";38;2;{};{};{}", rgb.r, rgb.g, rgb.b);
                    }
                }

                match cell_style.bg {
                    Color::Named(named) => match named {
                        NamedColor::Black => sgr.push_str(";40"),
                        NamedColor::Red => sgr.push_str(";41"),
                        NamedColor::Green => sgr.push_str(";42"),
                        NamedColor::Yellow => sgr.push_str(";43"),
                        NamedColor::Blue => sgr.push_str(";44"),
                        NamedColor::Magenta => sgr.push_str(";45"),
                        NamedColor::Cyan => sgr.push_str(";46"),
                        NamedColor::White => sgr.push_str(";47"),
                        NamedColor::BrightBlack => sgr.push_str(";100"),
                        NamedColor::BrightRed => sgr.push_str(";101"),
                        NamedColor::BrightGreen => sgr.push_str(";102"),
                        NamedColor::BrightYellow => sgr.push_str(";103"),
                        NamedColor::BrightBlue => sgr.push_str(";104"),
                        NamedColor::BrightMagenta => sgr.push_str(";105"),
                        NamedColor::BrightCyan => sgr.push_str(";106"),
                        NamedColor::BrightWhite => sgr.push_str(";107"),
                        NamedColor::Background => {},
                        _ => {},
                    },
                    Color::Indexed(idx) => {
                        let _ = write!(sgr, ";48;5;{}", idx);
                    }
                    Color::Spec(rgb) => {
                        let _ = write!(sgr, ";48;2;{};{};{}", rgb.r, rgb.g, rgb.b);
                    }
                }

                sgr.push('m');
                out.push_str(&sgr);
                current_style = cell_style;
            }

            let ch = if cell.c == '\0' { ' ' } else { cell.c };
            out.push(ch);

            if let Some(zerowidth) = cell.zerowidth() {
                for &zc in zerowidth {
                    out.push(zc);
                }
            }
        }

        if current_style != StyleState::default() {
            out.push_str("\x1b[0m");
        }

        out
    }

    pub fn cursor_position(&self) -> Option<(usize, usize)> {
        let display_offset = self.term.grid().display_offset();
        let pt = self.term.grid().cursor.point;
        let visible_line = pt.line.0 + (display_offset as i32);
        if visible_line >= 0 && visible_line < self.size.lines as i32 {
            Some((pt.column.0, visible_line as usize))
        } else {
            None
        }
    }

    pub fn get_render_cell_cluster(&self, col: usize, line: usize) -> (char, Option<Vec<char>>, Color, Color) {
        let grid = self.term.grid();
        let display_line = Line(line as i32 - grid.display_offset() as i32);
        let cell = &grid[display_line][Column(col)];

        let c = if cell.flags.contains(Flags::HIDDEN) {
            ' '
        } else {
            cell.c
        };

        let zerowidth = if cell.flags.contains(Flags::HIDDEN) {
            None
        } else {
            cell.zerowidth().map(|s| s.to_vec())
        };

        let (mut fg, bg) = if cell.flags.contains(Flags::INVERSE) {
            (cell.bg, cell.fg)
        } else {
            (cell.fg, cell.bg)
        };

        if cell.flags.contains(Flags::BOLD) {
            match fg {
                Color::Named(NamedColor::Black) => fg = Color::Named(NamedColor::BrightBlack),
                Color::Named(NamedColor::Red) => fg = Color::Named(NamedColor::BrightRed),
                Color::Named(NamedColor::Green) => fg = Color::Named(NamedColor::BrightGreen),
                Color::Named(NamedColor::Yellow) => fg = Color::Named(NamedColor::BrightYellow),
                Color::Named(NamedColor::Blue) => fg = Color::Named(NamedColor::BrightBlue),
                Color::Named(NamedColor::Magenta) => fg = Color::Named(NamedColor::BrightMagenta),
                Color::Named(NamedColor::Cyan) => fg = Color::Named(NamedColor::BrightCyan),
                Color::Named(NamedColor::White) => fg = Color::Named(NamedColor::BrightWhite),
                _ => {}
            }
        }

        if cell.flags.contains(Flags::DIM) {
            match fg {
                Color::Named(NamedColor::Foreground) => fg = Color::Named(NamedColor::DimForeground),
                Color::Named(NamedColor::Black) => fg = Color::Named(NamedColor::DimBlack),
                Color::Named(NamedColor::Red) => fg = Color::Named(NamedColor::DimRed),
                Color::Named(NamedColor::Green) => fg = Color::Named(NamedColor::DimGreen),
                Color::Named(NamedColor::Yellow) => fg = Color::Named(NamedColor::DimYellow),
                Color::Named(NamedColor::Blue) => fg = Color::Named(NamedColor::DimBlue),
                Color::Named(NamedColor::Magenta) => fg = Color::Named(NamedColor::DimMagenta),
                Color::Named(NamedColor::Cyan) => fg = Color::Named(NamedColor::DimCyan),
                Color::Named(NamedColor::White) => fg = Color::Named(NamedColor::DimWhite),
                Color::Spec(rgb) => {
                    fg = Color::Spec(Rgb {
                        r: (rgb.r as u16 * 2 / 3) as u8,
                        g: (rgb.g as u16 * 2 / 3) as u8,
                        b: (rgb.b as u16 * 2 / 3) as u8,
                    });
                }
                _ => {}
            }
        }

        (c, zerowidth, fg, bg)
    }

    pub fn get_render_cell(&self, col: usize, line: usize) -> (char, Color, Color) {
        let (c, _zw, fg, bg) = self.get_render_cell_cluster(col, line);
        (c, fg, bg)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEventKind {
    Press,
    Drag,
    Move,
    Release,
}

pub fn format_sgr_mouse(
    btn: u8,
    col: usize,
    row: usize,
    kind: MouseEventKind,
    shift: bool,
    alt: bool,
    ctrl: bool,
) -> String {
    let mut code = match kind {
        MouseEventKind::Press => btn,
        MouseEventKind::Drag => btn + 32,
        MouseEventKind::Move => 35, // 3 + 32
        MouseEventKind::Release => btn,
    };
    if shift {
        code += 4;
    }
    if alt {
        code += 8;
    }
    if ctrl {
        code += 16;
    }

    let suffix = match kind {
        MouseEventKind::Release => 'm',
        _ => 'M',
    };

    format!("\x1b[<{};{};{}{}", code, col, row, suffix)
}

