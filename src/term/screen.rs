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

    pub fn get_cell_char(&self, col: usize, line: usize) -> char {
        let grid = self.term.grid();
        grid[Line(line as i32)][Column(col)].c
    }

    pub fn get_line_string(&self, line: usize) -> String {
        let grid = self.term.grid();
        let mut s = String::new();
        for col in 0..self.size.columns {
            s.push(grid[Line(line as i32)][Column(col)].c);
        }
        s.trim_end().to_string()
    }

    pub fn get_screen_text(&self) -> Vec<String> {
        (0..self.size.lines)
            .map(|l| self.get_line_string(l))
            .filter(|line| !line.is_empty())
            .collect()
    }
}
