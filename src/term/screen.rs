use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::vte::ansi::Processor;

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

pub struct TermScreen {
    pub term: Term<VoidListener>,
    processor: Processor,
    pub size: TermSize,
    pub dirty: bool,
}

impl TermScreen {
    pub fn new(columns: usize, lines: usize) -> Self {
        let size = TermSize { columns, lines };
        let term = Term::new(Config::default(), &size, VoidListener);
        let processor = Processor::default();

        Self {
            term,
            processor,
            size,
            dirty: true,
        }
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

    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    pub fn is_alt_screen(&self) -> bool {
        self.term.mode().contains(alacritty_terminal::term::TermMode::ALT_SCREEN)
    }

    pub fn is_bracketed_paste(&self) -> bool {
        self.term.mode().contains(alacritty_terminal::term::TermMode::BRACKETED_PASTE)
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

    pub fn get_render_cell(&self, col: usize, line: usize) -> (char, alacritty_terminal::vte::ansi::Color, alacritty_terminal::vte::ansi::Color) {
        let grid = self.term.grid();
        let display_line = Line(line as i32 - grid.display_offset() as i32);
        let cell = &grid[display_line][Column(col)];
        (cell.c, cell.fg, cell.bg)
    }
}
