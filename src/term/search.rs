use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::grid::Dimensions;
use crate::term::TermScreen;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchHighlightType {
    Normal,
    Active,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub line: Line,
    pub start_col: usize,
    pub end_col: usize, // exclusive column index
}

#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub is_open: bool,
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub current_match_idx: Option<usize>,
}

impl SearchState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
        self.matches.clear();
        self.current_match_idx = None;
    }

    pub fn toggle(&mut self) {
        if self.is_open {
            self.close();
        } else {
            self.open();
        }
    }

    pub fn set_query(&mut self, query: String, screen: &TermScreen) {
        self.query = query;
        self.update_matches(screen);
    }

    pub fn update_matches(&mut self, screen: &TermScreen) {
        if self.query.is_empty() {
            self.matches.clear();
            self.current_match_idx = None;
            return;
        }

        self.matches = find_matches(screen, &self.query);
        if self.matches.is_empty() {
            self.current_match_idx = None;
        } else {
            // Keep current match index if within bounds, otherwise select first match
            if let Some(idx) = self.current_match_idx {
                if idx >= self.matches.len() {
                    self.current_match_idx = Some(0);
                }
            } else {
                self.current_match_idx = Some(0);
            }
        }
    }

    pub fn current_match(&self) -> Option<&SearchMatch> {
        self.current_match_idx.and_then(|idx| self.matches.get(idx))
    }

    pub fn next_match(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            self.current_match_idx = None;
            return None;
        }
        let next_idx = match self.current_match_idx {
            Some(idx) => (idx + 1) % self.matches.len(),
            None => 0,
        };
        self.current_match_idx = Some(next_idx);
        self.matches.get(next_idx)
    }

    pub fn prev_match(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            self.current_match_idx = None;
            return None;
        }
        let prev_idx = match self.current_match_idx {
            Some(idx) => {
                if idx == 0 {
                    self.matches.len() - 1
                } else {
                    idx - 1
                }
            }
            None => self.matches.len() - 1,
        };
        self.current_match_idx = Some(prev_idx);
        self.matches.get(prev_idx)
    }

    /// Check if a cell at (line, col) is part of any match, returning highlight type if matched.
    pub fn check_cell_highlight(&self, line: Line, col: usize) -> Option<MatchHighlightType> {
        if !self.is_open || self.matches.is_empty() {
            return None;
        }

        // Active match takes visual precedence
        if let Some(am) = self.current_match() {
            if am.line == line && col >= am.start_col && col < am.end_col {
                return Some(MatchHighlightType::Active);
            }
        }

        // Check if point falls within any other match
        for m in &self.matches {
            if m.line == line && col >= m.start_col && col < m.end_col {
                return Some(MatchHighlightType::Normal);
            }
        }

        None
    }

    /// Calculate target display_offset to bring the current match into view.
    pub fn scroll_offset_for_current_match(&self, visible_lines: usize, history_size: usize) -> Option<usize> {
        let m = self.current_match()?;
        Some(calculate_scroll_offset_for_line(m.line, visible_lines, history_size))
    }

    /// Format counter string e.g. "1/14" or "0/0"
    pub fn match_counter_string(&self) -> String {
        let total = self.matches.len();
        if total == 0 {
            "0/0".to_string()
        } else {
            let current = self.current_match_idx.map(|idx| idx + 1).unwrap_or(0);
            format!("{}/{}", current, total)
        }
    }
}

pub fn calculate_scroll_offset_for_line(line: Line, visible_lines: usize, history_size: usize) -> usize {
    // When display_offset = D, viewport displays lines from -D to visible_lines - 1 - D.
    // We want the target line to appear around vertical center (upper third) of viewport.
    let center_row = (visible_lines / 3) as i32;
    // center_row - D = line.0  =>  D = center_row - line.0
    let desired_offset = center_row - line.0;
    desired_offset.clamp(0, history_size as i32) as usize
}

pub fn find_matches(screen: &TermScreen, query: &str) -> Vec<SearchMatch> {
    if query.is_empty() {
        return Vec::new();
    }

    let grid = screen.term.grid();
    let total_lines = grid.total_lines();
    let visible_lines = grid.screen_lines();
    let history_lines = total_lines.saturating_sub(visible_lines);
    let cols = screen.size.columns;

    let query_lower = query.to_lowercase();
    let query_char_count = query_lower.chars().count();
    if query_char_count == 0 {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let min_line = -(history_lines as i32);
    let max_line = visible_lines as i32;

    for l in min_line..max_line {
        let line_idx = Line(l);
        let row = &grid[line_idx];

        let mut char_cols: Vec<(char, usize, usize)> = Vec::with_capacity(cols);
        let mut row_str = String::with_capacity(cols);

        let mut c = 0;
        while c < cols {
            let cell = &row[Column(c)];
            if cell.flags.contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER) {
                c += 1;
                continue;
            }

            let ch = if cell.c == '\0' { ' ' } else { cell.c };
            let is_wide = cell.flags.contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR);
            let span = if is_wide { 2 } else { 1 };

            char_cols.push((ch, c, c + span));
            row_str.push(ch);

            c += span;
        }

        if row_str.trim_end().is_empty() {
            continue;
        }

        let row_lower = row_str.to_lowercase();
        let char_byte_indices: Vec<(usize, char)> = row_lower.char_indices().collect();

        for (byte_idx, _) in row_lower.match_indices(&query_lower) {
            if let Some(start_char_idx) = char_byte_indices.iter().position(|&(b, _)| b == byte_idx) {
                let end_char_idx = start_char_idx + query_char_count;
                if start_char_idx < char_cols.len() && end_char_idx <= char_cols.len() {
                    let start_col = char_cols[start_char_idx].1;
                    let end_col = char_cols[end_char_idx - 1].2;
                    matches.push(SearchMatch {
                        line: line_idx,
                        start_col,
                        end_col,
                    });
                }
            }
        }
    }

    matches
}
