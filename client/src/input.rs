use std::ops::{Deref, DerefMut};
use unicode_width::UnicodeWidthChar;

/// A structure that holds input strings
pub struct History {
    /// The list of all history
    history: Vec<Input>,
    /// Current index
    index: usize,
}

impl History {
    /// Make a new history struct
    pub fn new() -> Self {
        let mut h = History {
            history: Vec::with_capacity(100),
            index: 0,
        };
        h.history.push(Input::new());
        h
    }

    /// Increment the history to the next Input
    pub fn next(&mut self) {
        let max = match self.history.len().checked_sub(1) {
            Some(n) => n,
            None => 0,
        };
        self.index = match self.index.checked_add(1) {
            Some(n) => std::cmp::min(n, max),
            None => max,
        }
    }

    /// Decrement the history to the previous Input
    pub fn prev(&mut self) {
        self.index = match self.index.checked_sub(1) {
            Some(n) => n,
            None => 0,
        }
    }

    /// Append a new Input to the list and set it to the current input
    pub fn new_line(&mut self) {
        // Don't make a new entry if the last one is already empty
        let makenew = if let Some(input) = self.history.last() {
            !input.buffer.is_empty()
        } else {
            true
        };

        if makenew {
            self.history.push(Input::new());
        }

        self.index = match self.history.len().checked_sub(1) {
            Some(n) => n,
            None => 0,
        }
    }
}

impl Deref for History {
    type Target = Input;

    fn deref(&self) -> &Self::Target {
        self.history.get(self.index).unwrap()
    }
}

impl DerefMut for History {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.history.get_mut(self.index).unwrap()
    }
}

/// A struct that holds user input
pub struct Input {
    /// The input buffer
    buffer: Vec<char>,
    /// The current cursor position
    cursor: usize,
}

impl Input {
    /// Make new Input
    pub fn new() -> Self {
        Input {
            buffer: Vec::with_capacity(0),
            cursor: 0,
        }
    }

    /// Return a clone of the input buffer
    pub fn get_string(&self) -> String {
        self.buffer.iter().collect()
    }

    /// Return a view of the input that contains the cursor.
    /// Returns a tuple (data: String, cursor: usize) where the
    /// cursor is always within the provided view.
    pub fn view(&self, width: usize) -> (String, usize) {
        let quarter = match width.checked_div(4) {
            Some(m) => m,
            None => 1,
        };

        let max_cols = 3 * quarter;
        let mut want_cols = max_cols;
        let mut start_pos = self.cursor;
        while start_pos > 0 && want_cols > 0 {
            start_pos -= 1;
            if let Some(c) = self.buffer.get(start_pos) {
                if let Some(w) = UnicodeWidthChar::width(*c) {
                    want_cols = match want_cols.checked_sub(w) {
                        Some(new) => new,
                        None => 0,
                    }
                }
            }
        }

        let ret: String = self.buffer.iter().skip(start_pos).collect();
        (ret, max_cols - want_cols)
    }

    /// Set the cursor value to the given value or the maximum
    /// possible value at the end of the buffer.
    fn set_cursor(&mut self, new: usize) {
        let max = self.buffer.len();
        self.cursor = std::cmp::min(max, new);
    }

    /// Add a character to the buffer
    pub fn insert(&mut self, c: char) {
        self.buffer.insert(self.cursor, c);
        self.set_cursor(self.cursor.wrapping_add(1));
    }

    /// Delete a character from the buffer and decrement the cursor
    pub fn backspace(&mut self) {
        loop {
            if let Some(n) = self.cursor.checked_sub(1) {
                let removed = self.buffer.remove(n);
                self.cursor = n;
                match UnicodeWidthChar::width(removed) {
                    // Keep backspacing while we're deleting zero-width chars
                    Some(width) if width == 0 => continue,
                    _ => break,
                }
            } else {
                break;
            }
        }
    }

    /// Delete a character from the buffer and do not decrement the cursor
    pub fn delete(&mut self) {
        loop {
            if let None = self.buffer.get(self.cursor) {
                break;
            }
            self.buffer.remove(self.cursor);
            self.set_cursor(self.cursor);
            if !self.is_zero_width(self.cursor) {
                break;
            }
        }
    }

    /// Get the character before the cursor
    fn prev_char(&self) -> Option<&char> {
        match self.cursor.checked_sub(1) {
            Some(n) => self.buffer.get(n),
            None => None,
        }
    }

    /// Delete from cursor to end of previous word
    pub fn backspace_word(&mut self) {
        // If we are already on whitespace, back up to the prev word
        loop {
            match self.prev_char() {
                Some(c) => {
                    if c.is_whitespace() {
                        self.backspace();
                    } else {
                        break;
                    }
                }
                None => break,
            }
        }

        // Now delete the word
        loop {
            match self.prev_char() {
                Some(c) => {
                    if c.is_whitespace() {
                        break;
                    } else {
                        self.backspace();
                    }
                }
                None => break,
            }
        }
    }

    /// Return true if the char at the index exists and is zero width
    fn is_zero_width(&self, index: usize) -> bool {
        if let Some(c) = self.buffer.get(index) {
            if let Some(w) = UnicodeWidthChar::width(*c) {
                if w == 0 {
                    return true;
                }
            }
        }
        return false;
    }

    /// Move the cursor the the right
    pub fn move_right(&mut self, num: usize) {
        loop {
            match self.cursor.checked_add(num) {
                Some(n) => self.set_cursor(n),
                None => break,
            };
            if !self.is_zero_width(self.cursor) {
                break;
            }
        }
    }

    /// Move the cursor to the left
    pub fn move_left(&mut self, num: usize) {
        loop {
            self.cursor = match self.cursor.checked_sub(num) {
                Some(n) => n,
                None => 0,
            };
            if self.cursor == 0 || !self.is_zero_width(self.cursor) {
                break;
            }
        }
    }

    /// Move to the end of the input
    pub fn move_to_end(&mut self) {
        self.set_cursor(self.buffer.len());
    }

    /// Move to the start of the input
    pub fn move_to_start(&mut self) {
        self.cursor = 0;
    }
}
