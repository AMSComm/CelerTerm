use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::grid::Dimensions;
use unicode_normalization::UnicodeNormalization;
use crate::term::TermScreen;

pub const MAX_SEARCH_MATCHES: usize = 20000;

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
    pub match_group_id: usize, // group ID for unique occurrences (supports multiline wrapped matches)
}

#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub is_open: bool,
    pub query: String,
    pub cursor_pos: usize, // cursor character position within `query`
    pub matches: Vec<SearchMatch>,
    pub current_match_idx: Option<usize>, // current active `match_group_id`
    pub total_match_count: usize, // count of unique match groups
    pub is_capped: bool,
}

impl SearchState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.cursor_pos = self.query.chars().count();
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
        self.cursor_pos = 0;
        self.matches.clear();
        self.current_match_idx = None;
        self.total_match_count = 0;
        self.is_capped = false;
    }

    pub fn toggle(&mut self) {
        if self.is_open {
            self.close();
        } else {
            self.open();
        }
    }

    pub fn move_cursor_left(&mut self) {
        self.cursor_pos = self.cursor_pos.saturating_sub(1);
    }

    pub fn move_cursor_right(&mut self) {
        let max_pos = self.query.chars().count();
        if self.cursor_pos < max_pos {
            self.cursor_pos += 1;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.query.chars().count();
    }

    pub fn insert_char(&mut self, c: char) {
        let byte_pos = self.query.char_indices()
            .nth(self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.query.len());
        self.query.insert(byte_pos, c);
        self.cursor_pos += 1;
    }

    pub fn insert_str(&mut self, text: &str) {
        let byte_pos = self.query.char_indices()
            .nth(self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.query.len());
        self.query.insert_str(byte_pos, text);
        self.cursor_pos += text.chars().count();
    }

    pub fn delete_backward(&mut self) -> bool {
        if self.cursor_pos == 0 {
            return false;
        }
        let target_pos = self.cursor_pos - 1;
        if let Some((byte_pos, _)) = self.query.char_indices().nth(target_pos) {
            self.query.remove(byte_pos);
            self.cursor_pos -= 1;
            true
        } else {
            false
        }
    }

    pub fn delete_forward(&mut self) -> bool {
        if let Some((byte_pos, _)) = self.query.char_indices().nth(self.cursor_pos) {
            self.query.remove(byte_pos);
            true
        } else {
            false
        }
    }

    pub fn set_query(&mut self, query: String, screen: &TermScreen) {
        self.query = query;
        self.cursor_pos = self.query.chars().count();
        self.update_matches(screen);
    }

    pub fn update_matches(&mut self, screen: &TermScreen) {
        if self.query.is_empty() {
            self.matches.clear();
            self.current_match_idx = None;
            self.total_match_count = 0;
            self.is_capped = false;
            return;
        }

        let (matches, is_capped) = find_matches_with_cap(screen, &self.query, MAX_SEARCH_MATCHES);
        let total_groups = matches.last().map(|m| m.match_group_id + 1).unwrap_or(0);
        self.matches = matches;
        self.total_match_count = total_groups;
        self.is_capped = is_capped;

        if self.total_match_count == 0 {
            self.current_match_idx = None;
        } else {
            // Keep current match index if within bounds, otherwise select closest to current viewport
            if let Some(idx) = self.current_match_idx {
                if idx >= self.total_match_count {
                    self.current_match_idx = Some(0);
                }
            } else {
                let display_offset = screen.display_offset();
                let visible_lines = screen.size.lines;
                let current_top = -(display_offset as i32);
                let current_bottom = (visible_lines as i32 - 1) - display_offset as i32;

                // Prioritize a match already in the visible viewport
                let visible_match = self.matches.iter().find(|m| m.line.0 >= current_top && m.line.0 <= current_bottom);
                if let Some(vm) = visible_match {
                    self.current_match_idx = Some(vm.match_group_id);
                } else {
                    let closest = self.matches.iter().min_by_key(|m| (m.line.0 - current_top).abs());
                    self.current_match_idx = closest.map(|m| m.match_group_id).or(Some(0));
                }
            }
        }
    }

    pub fn current_match(&self) -> Option<&SearchMatch> {
        let active_gid = self.current_match_idx?;
        self.matches.iter().find(|m| m.match_group_id == active_gid)
    }

    pub fn next_match(&mut self) -> Option<&SearchMatch> {
        if self.total_match_count == 0 {
            self.current_match_idx = None;
            return None;
        }
        let next_gid = match self.current_match_idx {
            Some(gid) => (gid + 1) % self.total_match_count,
            None => 0,
        };
        self.current_match_idx = Some(next_gid);
        self.current_match()
    }

    pub fn prev_match(&mut self) -> Option<&SearchMatch> {
        if self.total_match_count == 0 {
            self.current_match_idx = None;
            return None;
        }
        let prev_gid = match self.current_match_idx {
            Some(gid) => {
                if gid == 0 {
                    self.total_match_count - 1
                } else {
                    gid - 1
                }
            }
            None => self.total_match_count - 1,
        };
        self.current_match_idx = Some(prev_gid);
        self.current_match()
    }

    /// O(log N) slice lookup for all match pieces on a specific line.
    pub fn matches_for_line(&self, line: Line) -> &[SearchMatch] {
        if self.matches.is_empty() {
            return &[];
        }
        let start = self.matches.partition_point(|m| m.line.0 < line.0);
        let len = self.matches[start..].partition_point(|m| m.line.0 == line.0);
        &self.matches[start..start + len]
    }

    /// Check if a cell at (line, col) is part of any match, returning highlight type if matched.
    pub fn check_cell_highlight(&self, line: Line, col: usize) -> Option<MatchHighlightType> {
        if !self.is_open || self.matches.is_empty() {
            return None;
        }

        let row_matches = self.matches_for_line(line);
        if row_matches.is_empty() {
            return None;
        }

        // Active match takes visual precedence
        if let Some(active_gid) = self.current_match_idx {
            for m in row_matches {
                if m.match_group_id == active_gid && col >= m.start_col && col < m.end_col {
                    return Some(MatchHighlightType::Active);
                }
            }
        }

        // Check if point falls within any match piece for this line
        for m in row_matches {
            if col >= m.start_col && col < m.end_col {
                return Some(MatchHighlightType::Normal);
            }
        }

        None
    }

    /// Calculate target display_offset to bring the current match into view.
    /// Returns None if the current match is already visible in the viewport.
    pub fn scroll_offset_for_current_match(&self, current_display_offset: usize, visible_lines: usize, history_size: usize) -> Option<usize> {
        let m = self.current_match()?;
        let current_top = -(current_display_offset as i32);
        let current_bottom = (visible_lines as i32 - 1) - current_display_offset as i32;

        if m.line.0 >= current_top && m.line.0 <= current_bottom {
            return None;
        }

        Some(calculate_scroll_offset_for_line(m.line, visible_lines, history_size))
    }

    /// Format counter string e.g. "1/14", "1/20000+" or "0/0"
    pub fn match_counter_string(&self) -> String {
        let total = if self.total_match_count > 0 {
            self.total_match_count
        } else {
            self.matches.last().map(|m| m.match_group_id + 1).unwrap_or(0)
        };
        let cap_suffix = if self.is_capped { "+" } else { "" };
        if total == 0 {
            "0/0".to_string()
        } else {
            let current = self.current_match_idx.map(|idx| idx + 1).unwrap_or(0);
            format!("{}/{}{}", current, total, cap_suffix)
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
    find_matches_with_cap(screen, query, MAX_SEARCH_MATCHES).0
}

pub fn find_matches_with_cap(screen: &TermScreen, query: &str, max_matches: usize) -> (Vec<SearchMatch>, bool) {
    if query.is_empty() {
        return (Vec::new(), false);
    }

    let query_normalized: String = query.nfc().collect();
    let query_lower = query_normalized.to_lowercase();
    let query_char_count = query_lower.chars().count();
    if query_char_count == 0 {
        return (Vec::new(), false);
    }

    let grid = screen.term.grid();
    let total_lines = grid.total_lines();
    let visible_lines = grid.screen_lines();
    let history_lines = total_lines.saturating_sub(visible_lines);
    let cols = screen.size.columns;

    let mut matches = Vec::new();
    let min_line = -(history_lines as i32);
    let max_line = visible_lines as i32;

    // Buffer to accumulate logical lines (which can span multiple wrapped rows).
    // Each char in logical line tracks: (norm_char, Line, start_col, end_col)
    let mut char_cols: Vec<(char, Line, usize, usize)> = Vec::with_capacity(cols * 2);
    let mut logical_str = String::with_capacity(cols * 2);
    let mut logical_lower = String::with_capacity(cols * 2);
    let mut char_byte_indices: Vec<(usize, char)> = Vec::with_capacity(cols * 2);
    let mut cell_buf = String::with_capacity(8);
    let mut is_capped = false;
    let mut group_id = 0;

    let mut l = min_line;
    'outer: while l < max_line {
        char_cols.clear();
        logical_str.clear();

        // Accumulate row `l` and any subsequent wrapped rows into this logical line
        loop {
            let line_idx = Line(l);
            let row = &grid[line_idx];
            let mut c = 0;

            let is_wrapped = cols > 0 && row[Column(cols - 1)].flags.contains(alacritty_terminal::term::cell::Flags::WRAPLINE);
            let max_c = if is_wrapped {
                cols
            } else {
                (0..cols).rposition(|col_idx| {
                    let cell = &row[Column(col_idx)];
                    cell.c != ' ' && cell.c != '\0'
                }).map(|idx| idx + 1).unwrap_or(0)
            };

            while c < max_c {
                let cell = &row[Column(c)];
                if cell.flags.contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER)
                    || cell.flags.contains(alacritty_terminal::term::cell::Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    c += 1;
                    continue;
                }

                let ch = if cell.c == '\0' { ' ' } else { cell.c };
                let is_wide = cell.flags.contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR);
                let span = if is_wide { 2 } else { 1 };
                let end_c = (c + span).min(cols);

                cell_buf.clear();
                cell_buf.push(ch);
                if let Some(zw) = cell.zerowidth() {
                    for &z in zw {
                        cell_buf.push(z);
                    }
                }

                // Normalize cell content to NFC so decomposed input matches composed search queries
                for norm_ch in cell_buf.nfc() {
                    char_cols.push((norm_ch, line_idx, c, end_c));
                    logical_str.push(norm_ch);
                }

                c += span;
            }

            l += 1;
            // If this line did not wrap or we reached the end of the grid, finish logical line
            if !is_wrapped || l >= max_line {
                break;
            }
        }

        if logical_str.is_empty() {
            continue;
        }

        logical_lower.clear();
        for ch in logical_str.chars() {
            for lc in ch.to_lowercase() {
                logical_lower.push(lc);
            }
        }

        char_byte_indices.clear();
        char_byte_indices.extend(logical_lower.char_indices());

        for (byte_idx, _) in logical_lower.match_indices(&query_lower) {
            if let Some(start_char_idx) = char_byte_indices.iter().position(|&(b, _)| b == byte_idx) {
                let end_char_idx = start_char_idx + query_char_count;
                if start_char_idx < char_cols.len() && end_char_idx <= char_cols.len() {
                    let start_entry = &char_cols[start_char_idx];
                    let end_entry = &char_cols[end_char_idx - 1];

                    if start_entry.1 == end_entry.1 {
                        // Entire match is on a single row
                        matches.push(SearchMatch {
                            line: start_entry.1,
                            start_col: start_entry.2,
                            end_col: end_entry.3,
                            match_group_id: group_id,
                        });
                    } else {
                        // Match spans across wrapped rows
                        let start_line = start_entry.1.0;
                        let end_line = end_entry.1.0;
                        for cur_line in start_line..=end_line {
                            let match_line = Line(cur_line);
                            let col_start = if cur_line == start_line { start_entry.2 } else { 0 };
                            let col_end = if cur_line == end_line { end_entry.3 } else { cols };
                            matches.push(SearchMatch {
                                line: match_line,
                                start_col: col_start,
                                end_col: col_end,
                                match_group_id: group_id,
                            });
                        }
                    }

                    group_id += 1;
                    if group_id >= max_matches {
                        is_capped = true;
                        break 'outer;
                    }
                }
            }
        }
    }

    (matches, is_capped)
}
