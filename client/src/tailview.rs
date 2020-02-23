use std::convert::TryFrom;
use tui::backend::Backend;
use tui::layout::Rect;
use tui::terminal::Frame;
use tui::widgets::{Block, Borders, Paragraph, Text, Widget};
use unicode_width::UnicodeWidthStr;

struct Line {
    text: String,
}

impl Line {
    fn new(line: String) -> Line {
        let mut add = line.trim_end().to_string();
        add.push('\n');
        Line { text: add }
    }

    fn height(&self, view_width_16: u16) -> u16 {
        let mut h: usize = 1;
        let mut w: usize = 0;
        let view_width: usize = view_width_16.into();

        self.text.split_whitespace().for_each(|e| {
            let mut next_w = e.width();

            // handle word wrapping
            if w + next_w > view_width && w > 0 {
                // will split on whitespace
                h += 1;
                w = 0;
            }

            // handle truncation
            while next_w > view_width {
                h += 1;
                next_w -= view_width;
            }

            // +1 for the space
            w += next_w + 1;
        });

        match u16::try_from(h) {
            Ok(res) => res,
            Err(_) => u16::max_value(),
        }
    }
}

// A Paragraph that follows its last entry and allows scrolling
pub struct TailView {
    // The full history for this view
    history: Vec<Line>,
    // Which history element to start drawing at
    start: usize,
    // The maximum line to start at
    max_start: usize,
}

impl TailView {
    pub fn new() -> TailView {
        TailView {
            history: Vec::with_capacity(1000),
            start: 0,
            max_start: 0,
        }
    }

    pub fn add(&mut self, line: String) {
        self.history.push(Line::new(line));
    }

    pub fn scroll_up(&mut self, rect: Rect) {
        let delta: usize = if rect.height > 1 { rect.height / 2 } else { 1 }.into();

        if let Some(res) = self.start.checked_sub(delta) {
            self.start = res;
        } else {
            self.start = 0;
        }
    }

    pub fn scroll_down(&mut self, rect: Rect) {
        let delta: usize = if rect.height > 1 { rect.height / 2 } else { 1 }.into();

        if let Some(res) = self.start.checked_add(delta) {
            self.start = res;
        }

        self.start = std::cmp::min(self.start, self.max_start);
    }

    fn update_max_start(&mut self, area: Rect) {
        let mut heights: Vec<u16> = self
            .history
            .iter()
            .skip(self.max_start)
            .map(|l| l.height(area.width))
            .collect();

        let mut height = heights.iter().sum();
        while area.height < height {
            if let Some(h) = heights.pop() {
                height -= h;
                self.auto_scroll();
            } else {
                break;
            }
        }
    }

    fn auto_scroll(&mut self) {
        let increment_start = self.start == self.max_start;

        if let Some(res) = self.max_start.checked_add(1) {
            self.max_start = res;
        }

        // sanity check - cap at the last line of history
        if !self.history.is_empty() {
            self.max_start = std::cmp::min(self.max_start, self.history.len() - 1);
        }

        if increment_start {
            self.start = self.max_start;
        }
    }

    pub fn draw<B>(&mut self, mut frame: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {
        let b = Block::default().borders(Borders::TOP);

        self.update_max_start(b.inner(area));

        let lines: Vec<Text> = self
            .history
            .iter()
            .skip(self.start)
            .map(|l| Text::raw(&l.text))
            .collect();

        Paragraph::new(lines.iter())
            .block(b)
            .wrap(true)
            .render(&mut frame, area);
    }
}
