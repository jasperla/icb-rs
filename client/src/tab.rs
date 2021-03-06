use chrono::Local;
use std::convert::TryFrom;
use std::io::Error;
use std::path::PathBuf;
use tui::backend::Backend;
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::style::{Modifier, Style};
use tui::terminal::Frame;
use tui::widgets::{Block, Borders, Paragraph, Text, Widget};

use crate::message::{Message, MessageType};
use crate::tailview::TailView;
use icb::Command;

#[derive(Clone, PartialEq)]
pub enum ChatType {
    Status(String),
    Open(String),
    Personal(String),
}

const STATUS: &str = "Status";

struct Tab {
    view: TailView,
    title: String,
    tab_type: ChatType,
    has_unread: bool,
}

impl Tab {
    fn new(tab_type: ChatType, log_path: Option<PathBuf>) -> Tab {
        match tab_type {
            ChatType::Status(ref name)
            | ChatType::Open(ref name)
            | ChatType::Personal(ref name) => Tab {
                view: TailView::new(name, log_path),
                title: name.clone(),
                tab_type,
                has_unread: false,
            },
        }
    }

    fn add(&mut self, message: Message) -> Result<(), String> {
        self.view.add(message);
        self.has_unread = true;
        Ok(())
    }

    fn add_read(&mut self, message: Message) -> Result<(), String> {
        self.view.add(message);
        Ok(())
    }

    fn title(&self) -> Text {
        let mut modifier = Modifier::empty();
        if self.has_unread {
            modifier.insert(Modifier::BOLD);
            modifier.insert(Modifier::UNDERLINED);
        };

        Text::Styled(
            self.title.clone().into(),
            Style::default().modifier(modifier),
        )
    }

    fn command(&self, msg: &str) -> Command {
        match self.tab_type {
            ChatType::Personal(ref user) => Command::Personal(user.clone(), msg.to_string()),
            _ => Command::Open(msg.to_string()),
        }
    }
}

pub struct Tabs {
    tabs: Vec<Tab>,
    current_tab: usize,
    log_path: Option<PathBuf>,
    log_default: bool,
}

impl Tabs {
    pub fn new() -> Tabs {
        let mut v = Vec::new();
        v.push(Tab::new(ChatType::Status(STATUS.to_string()), None));

        Tabs {
            tabs: v,
            current_tab: 0,
            log_path: None,
            log_default: false,
        }
    }

    pub fn set_logging(&mut self, path: Option<PathBuf>, default: bool) {
        self.log_path = path;
        self.log_default = default;
    }

    pub fn add_message(&mut self, to: ChatType, msg: Message) -> Result<(), String> {
        for t in &mut self.tabs {
            if t.tab_type == to {
                t.add(msg)?;
                return Ok(());
            }
        }

        // New chat
        let mut newtab = Tab::new(to.clone(), self.log_path.clone());

        // Enable logging if needed. Defer handling the result until
        // everything is set up, since a log error is not fatal.
        let log_res = if self.log_default {
            newtab.view.enable_logging().map_err(|why| why.to_string())
        } else {
            Ok(())
        };

        newtab.add(msg)?;
        self.tabs.push(newtab);

        // If it's a new group chat, then switch to it
        if let ChatType::Open(_) = to {
            self.current_tab = self.tabs.len() - 1;
        }
        log_res
    }

    pub fn command_for_current(&self, msg: &str) -> Command {
        if let Some(tab) = self.tabs.get(self.current_tab) {
            tab.command(msg)
        } else {
            // This shouldn't happen.
            Command::Open(msg.to_string())
        }
    }

    pub fn add_current(&mut self, msg: Message) -> Result<(), String> {
        if let Some(tab) = self.tabs.get_mut(self.current_tab) {
            tab.add_read(msg)
        } else {
            Err("No current tab?".to_string())
        }
    }

    pub fn switch_to(&mut self, to: ChatType) {
        if let Some(n) = self.tabs.iter().position(|e| e.tab_type == to) {
            self.current_tab = n;
        }
    }

    pub fn add_status(&mut self, msg: String) -> Result<(), String> {
        self.add_message(
            ChatType::Status(STATUS.to_string()),
            Message::new(
                Local::now(),
                MessageType::Status,
                "[system]".to_string(),
                msg,
            ),
        )
    }

    pub fn draw_titles<B>(&mut self, mut frame: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {
        let n: u32 = match u32::try_from(self.tabs.len()) {
            Ok(l) => l,
            Err(_) => u32::max_value(),
        };
        let constraints = vec![Constraint::Ratio(1, n); n as usize];

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area)
            .iter()
            .enumerate()
            .for_each(|(i, area)| {
                let is_cur = i == self.current_tab;
                let mut borders = Borders::NONE;
                if !is_cur {
                    borders |= Borders::BOTTOM;
                    borders |= Borders::LEFT;
                    borders |= Borders::RIGHT;
                }

                Paragraph::new([self.tabs[i].title()].iter())
                    .block(Block::default().borders(borders))
                    .alignment(Alignment::Center)
                    .render(&mut frame, *area);
            });
    }

    pub fn draw_current<B>(&mut self, mut frame: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {
        if let Some(tab) = self.tabs.get_mut(self.current_tab) {
            tab.has_unread = false;
            tab.view.draw(&mut frame, area);
        }
    }

    pub fn scroll_up(&mut self, area: Rect) {
        if let Some(tab) = self.tabs.get_mut(self.current_tab) {
            tab.view.scroll_up(area);
        }
    }

    pub fn scroll_down(&mut self, area: Rect) {
        if let Some(tab) = self.tabs.get_mut(self.current_tab) {
            tab.view.scroll_down(area);
        }
    }

    pub fn next(&mut self) {
        self.current_tab = if !self.tabs.is_empty() {
            (self.current_tab + 1) % self.tabs.len()
        } else {
            0
        }
    }

    pub fn previous(&mut self) {
        self.current_tab = if self.current_tab > 0 {
            self.current_tab - 1
        } else {
            self.tabs.len() - 1
        }
    }

    pub fn toggle_show_date(&mut self) {
        if let Some(t) = self.tabs.get_mut(self.current_tab) {
            t.view.toggle_show_date();
        }
    }

    pub fn toggle_show_arrivals_departures(&mut self) {
        if let Some(t) = self.tabs.get_mut(self.current_tab) {
            t.view.toggle_show_arrivals();
            t.view.toggle_show_departures();
        }
    }

    pub fn toggle_autoscroll(&mut self) {
        if let Some(t) = self.tabs.get_mut(self.current_tab) {
            t.view.toggle_autoscroll();
        }
    }

    pub fn toggle_logging(&mut self) -> Result<(), Error> {
        if let Some(t) = self.tabs.get_mut(self.current_tab) {
            t.view.toggle_logging()
        } else {
            Ok(())
        }
    }

    pub fn status_line(&self) -> String {
        if let Some(t) = self.tabs.get(self.current_tab) {
            t.view.status_line()
        } else {
            String::new()
        }
    }
}
