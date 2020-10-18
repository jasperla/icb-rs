use crate::message::Message;
use chrono::Local;
use std::convert::TryFrom;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Write};
use std::path::PathBuf;
use tui::backend::Backend;
use tui::layout::Rect;
use tui::terminal::Frame;
use tui::widgets::{Block, Paragraph, Text, Widget};
use unicode_width::UnicodeWidthStr;

struct Line {
    message: Message,
}

impl Line {
    fn new(message: Message) -> Line {
        Line { message }
    }

    fn height(&self, view_options: &ViewOptions, view_width_16: u16) -> u16 {
        let mut h: usize = 1;
        let mut w: usize = 0;
        let view_width: usize = view_width_16.into();

        let text = self.message.render(view_options);

        if text.is_none() {
            return 0;
        }

        text.unwrap().split_whitespace().for_each(|e| {
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

// Options that control how this view renders and behaves
pub struct ViewOptions {
    pub show_date: bool,
    pub show_arrivals: bool,
    pub show_departures: bool,
    pub autoscroll: bool,
}

impl ViewOptions {
    pub fn new() -> Self {
        Self {
            show_date: false,
            show_arrivals: true,
            show_departures: true,
            autoscroll: true,
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
    // The view options
    options: ViewOptions,
    // The name of the room
    name: String,
    // The base path to the log file
    log_path: Option<PathBuf>,
    // The log file, if one if open
    log: Option<File>,
}

impl TailView {
    pub fn new(name: &String, log_path: Option<PathBuf>) -> TailView {
        TailView {
            history: Vec::with_capacity(1000),
            start: 0,
            max_start: 0,
            options: ViewOptions::new(),
            name: name.clone(),
            log_path: log_path.clone(),
            log: None,
        }
    }

    pub fn add(&mut self, message: Message) {
        if let Some(ref mut log) = self.log {
            if let Some(s) = message.render(&self.options) {
                log.write_all(&s.as_bytes()).ok();
            }
        }
        self.history.push(Line::new(message));
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
            .map(|l| l.height(&self.options, area.width))
            .rev()
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
        let increment_start = self.options.autoscroll && self.start == self.max_start;

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
        let b = Block::default();

        self.update_max_start(b.inner(area));

        let lines: Vec<Text> = self
            .history
            .iter()
            .skip(self.start)
            .filter_map(|l| l.message.render(&self.options))
            .map(|s| Text::raw(s))
            .collect();

        Paragraph::new(lines.iter())
            .block(b)
            .wrap(true)
            .render(&mut frame, area);
    }

    fn new_log(&mut self) -> Result<(), Error> {
        if let Some(base) = &self.log_path {
            let mut path = base.clone();
            path.push(self.name.clone());
            std::fs::create_dir_all(&path)?;
            path.push(Local::now().to_string());
            self.log = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .write(true)
                    .open(path)?,
            );
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::NotFound,
                "No log path found. This could mean failure to find $HOME",
            ))
        }
    }

    pub fn toggle_logging(&mut self) -> Result<(), Error> {
        if self.log.take().is_none() {
            self.new_log()
        } else {
            Ok(())
        }
    }

    pub fn enable_logging(&mut self) -> Result<(), Error> {
        self.new_log()
    }

    pub fn toggle_show_date(&mut self) {
        self.options.show_date = !self.options.show_date;
    }

    pub fn toggle_show_arrivals(&mut self) {
        self.options.show_arrivals = !self.options.show_arrivals;
    }

    pub fn toggle_show_departures(&mut self) {
        self.options.show_departures = !self.options.show_departures;
    }

    pub fn toggle_autoscroll(&mut self) {
        self.options.autoscroll = !self.options.autoscroll;
    }

    pub fn status_line(&self) -> String {
        let mut s = String::new();
        if !self.options.autoscroll {
            s.push('S');
        }
        if self.log.is_some() {
            s.push('L');
        }
        s
    }
}
