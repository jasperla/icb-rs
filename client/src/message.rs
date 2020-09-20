use crate::tailview::ViewOptions;
use chrono::{DateTime, Local};

#[derive(Debug, PartialEq)]
pub enum MessageType {
    Arrive,
    Beep,
    Boot,
    Depart,
    Help,
    Name,
    NoBeep,
    Notify,
    Open,
    Personal,
    SignOff,
    SignOn,
    Status,
    Topic,
    Unknown,
    Warning,
}

impl MessageType {
    // Parse status packet strings
    pub fn from_status_str(s: &str) -> Self {
        match s {
            "Arrive" => Self::Arrive,
            "Boot" => Self::Boot,
            "Depart" => Self::Depart,
            "Help" => Self::Help,
            "Name" => Self::Name,
            "No-Beep" => Self::NoBeep,
            "Notify" => Self::Notify,
            "Sign-off" => Self::SignOff,
            "Sign-on" => Self::SignOn,
            "Status" => Self::Status,
            "Topic" => Self::Topic,
            "Warning" => Self::Warning,
            _ => Self::Unknown,
        }
    }
}

pub struct Message {
    received: DateTime<Local>,
    message_type: MessageType,
    from: String,
    body: String,
}

impl Message {
    pub fn new(
        received: DateTime<Local>,
        message_type: MessageType,
        from: String,
        body: String,
    ) -> Self {
        Self {
            received,
            message_type,
            from,
            body,
        }
    }

    pub fn render(&self, opts: &ViewOptions) -> Option<String> {
        if !opts.show_arrivals
            && (self.message_type == MessageType::Arrive
                || self.message_type == MessageType::SignOn)
        {
            return None;
        }

        if !opts.show_departures
            && (self.message_type == MessageType::Depart
                || self.message_type == MessageType::SignOff)
        {
            return None;
        }

        let datestr = if opts.show_date {
            self.received.format("%b-%d %H:%M")
        } else {
            self.received.format("%H:%M")
        };

        let text = match self.message_type {
            MessageType::Open | MessageType::Personal | MessageType::Beep => {
                format!("{}: <{}> {}\n", datestr, self.from, self.body)
            }
            _ => format!("{}: {}\n", datestr, self.body),
        };

        Some(text)
    }
}
