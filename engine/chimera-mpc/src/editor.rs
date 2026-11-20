//! The touchscreen lyrics editor: a draft the on-screen keyboard edits (a cursor, one-shot shift,
//! backspace, clear, line breaks) and the applied text that Done hands to the singer. Pure logic,
//! run on the planner thread; key codes come from the plugin's keyboard parameters.

/// Key codes: printable ASCII inserts itself; these are the control keys.
pub const K_SHIFT: u8 = 1;
pub const K_BACK: u8 = 2;
pub const K_CLEAR: u8 = 3;
pub const K_ENTER: u8 = 4;
pub const K_LEFT: u8 = 5;
pub const K_RIGHT: u8 = 6;
/// copy the selected lyrics.txt set into the draft
pub const K_LOAD: u8 = 7;
/// throw the draft away: back to the applied text
pub const K_REVERT: u8 = 8;
pub const K_DONE: u8 = 9;

/// Longest draft (characters): about four minutes of syllables.
pub const MAX_CHARS: usize = 4000;
/// Longest screen line, in bytes: the MPC SDK's VST2 shell hands the host at most 31 bytes of a
/// parameter's display text (`kStr` in framework/mpc_plugin.h), so every line the screen shows
/// is built to fit (1.3.1; before, the draft line was cut at 31 bytes and the cursor fell off it).
pub const LINE_BYTES: usize = 30;
/// Lines of the draft the keyboard shows around the cursor.
pub const ROWS: usize = 3;

/// What a key asks of the instrument beyond editing the draft.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// the draft changed (or the cursor/shift did): refresh the display
    Edited,
    /// apply the draft as the lyrics
    Done,
    /// the caller should put the selected file set's text into the draft
    Load,
    Nothing,
}

#[derive(Default, Clone, Debug)]
pub struct Editor {
    draft: Vec<char>,
    cursor: usize,
    shift: bool,
    applied: String,
    /// Clear All was pressed once: the next Clear erases the draft, any other key cancels it
    /// (not saved: a reloaded project never comes back half-way through erasing)
    clear_armed: bool,
}

impl Editor {
    pub fn applied(&self) -> &str {
        &self.applied
    }

    pub fn draft(&self) -> String {
        self.draft.iter().collect()
    }

    /// New applied text (a restored project); the draft follows it.
    pub fn set_applied(&mut self, text: &str) {
        self.applied = text.chars().take(MAX_CHARS).collect();
        self.set_draft(&self.applied.clone());
    }

    pub fn set_draft(&mut self, text: &str) {
        self.draft = text.replace("\r\n", "\n").chars().filter(|c| *c != '\r').take(MAX_CHARS).collect();
        self.cursor = self.draft.len();
        self.shift = false;
    }

    pub fn key(&mut self, code: u8) -> Outcome {
        let armed = std::mem::take(&mut self.clear_armed);
        match code {
            K_SHIFT => self.shift = !self.shift,
            K_BACK => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.draft.remove(self.cursor);
                }
            }
            // protected: the first press only asks (the screen says so), the second erases
            K_CLEAR if armed => {
                self.draft.clear();
                self.cursor = 0;
            }
            K_CLEAR => self.clear_armed = !self.draft.is_empty(),
            K_ENTER => self.insert('\n'),
            K_LEFT => self.cursor = self.cursor.saturating_sub(1),
            K_RIGHT => self.cursor = (self.cursor + 1).min(self.draft.len()),
            K_LOAD => return Outcome::Load,
            K_REVERT => {
                let a = self.applied.clone();
                self.set_draft(&a);
            }
            K_DONE => {
                self.applied = self.draft();
                self.shift = false;
                return Outcome::Done;
            }
            0x20..=0x7E => {
                let mut c = code as char;
                if self.shift {
                    c = c.to_ascii_uppercase();
                    self.shift = false;
                }
                self.insert(c);
            }
            _ => return Outcome::Nothing,
        }
        Outcome::Edited
    }

    /// The whole editor (sung text, draft, cursor, shift) as bytes, for the project's state.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let draft = self.draft();
        for s in [self.applied.as_str(), draft.as_str()] {
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        }
        out.extend_from_slice(&(self.cursor as u32).to_le_bytes());
        out.push(self.shift as u8);
        out
    }

    /// Back from `to_bytes` (anything else: None).
    pub fn from_bytes(b: &[u8]) -> Option<Editor> {
        let mut pos = 0usize;
        let mut take = |n: usize| -> Option<&[u8]> {
            let s = b.get(pos..pos.checked_add(n)?)?;
            pos += n;
            Some(s)
        };
        let mut text = || -> Option<String> {
            let n = u32::from_le_bytes(take(4)?.try_into().ok()?) as usize;
            Some(String::from_utf8_lossy(take(n)?).into_owned())
        };
        let applied = text()?;
        let draft = text()?;
        let cursor = u32::from_le_bytes(take(4)?.try_into().ok()?) as usize;
        let shift = take(1)?[0] != 0;
        let mut e = Editor::default();
        e.applied = applied.chars().take(MAX_CHARS).collect();
        e.set_draft(&draft);
        e.cursor = cursor.min(e.draft.len());
        e.shift = shift;
        Some(e)
    }

    /// The draft is what is being sung (Done was pressed, nothing typed since).
    pub fn is_sung(&self) -> bool {
        !self.draft.is_empty() && self.draft.iter().copied().eq(self.applied.chars())
    }

    /// Changes whenever what the screen shows changes (draft, cursor, shift, sung): FNV-1a.
    pub fn fingerprint(&self) -> u32 {
        let mut h: u32 = 0x811C_9DC5;
        let mut mix = |x: u32| {
            for b in x.to_le_bytes() {
                h ^= b as u32;
                h = h.wrapping_mul(0x0100_0193);
            }
        };
        for c in &self.draft {
            mix(*c as u32);
        }
        mix(self.cursor as u32);
        mix(self.shift as u32 | 0x100);
        mix(self.is_sung() as u32 | 0x200);
        mix(self.clear_armed as u32 | 0x400);
        h
    }

    fn insert(&mut self, c: char) {
        if self.draft.len() < MAX_CHARS {
            self.draft.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    /// The keyboard's status line (at most `LINE_BYTES`): "(sung)" once Done has handed the
    /// draft to the singer, "(draft)" while it differs; its length; SHIFT while armed; and the
    /// Clear All question while it waits for its second press.
    pub fn status(&self) -> String {
        if self.clear_armed {
            return "TAP CLEAR ALL AGAIN TO ERASE".into();
        }
        let n = self.draft.len();
        let mut s = if n == 0 {
            "empty: type your words".to_string()
        } else {
            format!("{} {} chars", if self.is_sung() { "(sung)" } else { "(draft)" }, n)
        };
        if self.shift {
            s.push_str("  SHIFT");
        }
        s
    }

    /// The draft as `rows` screen lines of at most `width` bytes each, word-wrapped, with the
    /// cursor as "|" (Enter starts a new line), scrolled so that the cursor's line shows: second
    /// from the top when there is text below it, else the last. Always `rows` strings.
    pub fn lines(&self, width: usize, rows: usize) -> Vec<String> {
        let width = width.max(8);
        let mut text: Vec<char> = Vec::with_capacity(self.draft.len() + 1);
        text.extend_from_slice(&self.draft[..self.cursor]);
        text.push('|');
        text.extend_from_slice(&self.draft[self.cursor..]);
        let caret = self.cursor;
        // (line, does it hold the caret)
        let mut out: Vec<(String, bool)> = Vec::new();
        for (para_start, para) in split_keep_index(&text, '\n') {
            let mut line = String::new();
            let mut has = false;
            let mut i = 0;
            while i < para.len() {
                // the next word, with the spaces after it
                let mut j = i;
                while j < para.len() && para[j] != ' ' {
                    j += 1;
                }
                while j < para.len() && para[j] == ' ' {
                    j += 1;
                }
                let word: String = para[i..j].iter().collect();
                let word_has = (para_start + i..para_start + j).contains(&caret);
                if line.len() + word.trim_end().len() > width && !line.is_empty() {
                    out.push((std::mem::take(&mut line), has));
                    has = false;
                }
                if word.trim_end().len() > width {
                    // a word longer than a line: cut it at character boundaries
                    for (k, c) in para[i..j].iter().enumerate() {
                        if line.len() + c.len_utf8() > width {
                            out.push((std::mem::take(&mut line), has));
                            has = false;
                        }
                        line.push(*c);
                        has |= para_start + i + k == caret;
                    }
                } else {
                    line.push_str(&word);
                    has |= word_has;
                }
                i = j;
            }
            has |= para.is_empty() && para_start == caret;
            out.push((line, has));
        }
        // trailing spaces don't count against the width (they are invisible at a line's end)
        let lines: Vec<String> = out.iter().map(|(l, _)| trim_to(l, width)).collect();
        let at = out.iter().position(|(_, h)| *h).unwrap_or(lines.len().saturating_sub(1));
        let first = at.saturating_sub(1).min(lines.len().saturating_sub(rows));
        (0..rows).map(|r| lines.get(first + r).cloned().unwrap_or_default()).collect()
    }
}

/// `s`'s paragraphs between `sep`s, each with the index of its first character.
fn split_keep_index(s: &[char], sep: char) -> Vec<(usize, &[char])> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in s.iter().enumerate() {
        if *c == sep {
            out.push((start, &s[start..i]));
            start = i + 1;
        }
    }
    out.push((start, &s[start..]));
    out
}

/// `s` without trailing spaces past `width` bytes (a line's visible part always fits).
fn trim_to(s: &str, width: usize) -> String {
    let mut t = s.to_string();
    while t.len() > width && t.ends_with(' ') {
        t.pop();
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(e: &mut Editor, s: &str) {
        for b in s.bytes() {
            e.key(b);
        }
    }

    #[test]
    fn types_repeats_shift_space_and_punctuation() {
        let mut e = Editor::default();
        e.key(K_SHIFT);
        typed(&mut e, "hello, all!");
        assert_eq!(e.draft(), "Hello, all!");
        assert_eq!(e.lines(LINE_BYTES, ROWS), ["Hello, all!|", "", ""]);
        assert_eq!(e.status(), "(draft) 11 chars");
        // a letter twice is two letters (the plugin debounces the host, not the editor)
        typed(&mut e, "ll");
        assert_eq!(e.draft(), "Hello, all!ll");
    }

    #[test]
    fn backspace_cursor_clear_and_enter() {
        let mut e = Editor::default();
        typed(&mut e, "la la");
        e.key(K_LEFT);
        e.key(K_LEFT);
        e.key(K_BACK);
        typed(&mut e, "-");
        assert_eq!(e.draft(), "la-la");
        e.key(K_RIGHT);
        e.key(K_RIGHT);
        e.key(K_RIGHT);
        e.key(K_ENTER);
        typed(&mut e, "oh");
        assert_eq!(e.draft(), "la-la\noh");
        assert_eq!(e.lines(LINE_BYTES, ROWS), ["la-la", "oh|", ""]);
        // Clear All is protected: the first press asks, any other key cancels the question
        e.key(K_CLEAR);
        assert_eq!(e.draft(), "la-la\noh");
        assert_eq!(e.status(), "TAP CLEAR ALL AGAIN TO ERASE");
        e.key(K_LEFT);
        e.key(K_CLEAR);
        assert_eq!(e.draft(), "la-la\noh");
        e.key(K_RIGHT);
        assert_eq!(e.status(), "(draft) 8 chars");
        e.key(K_CLEAR);
        e.key(K_CLEAR);
        assert_eq!(e.draft(), "");
        e.key(K_CLEAR); // nothing to clear: nothing to ask either
        assert_eq!(e.status(), "empty: type your words");
        e.key(K_BACK); // nothing to delete: no panic
        assert_eq!(e.lines(LINE_BYTES, ROWS), ["|", "", ""]);
    }

    #[test]
    fn done_applies_and_revert_restores() {
        let mut e = Editor::default();
        typed(&mut e, "sing");
        assert_eq!(e.status(), "(draft) 4 chars");
        let f = e.fingerprint();
        assert_eq!(e.key(K_DONE), Outcome::Done);
        assert_eq!(e.applied(), "sing");
        // Done shows at once: the line says so, and its fingerprint (the host's redraw cue) moves
        assert_eq!(e.status(), "(sung) 4 chars");
        assert_ne!(e.fingerprint(), f);
        typed(&mut e, " more");
        e.key(K_REVERT);
        assert_eq!(e.draft(), "sing");
        assert_eq!(e.key(K_LOAD), Outcome::Load);
    }

    #[test]
    fn unicode_backspace_removes_whole_characters() {
        let mut e = Editor::default();
        e.set_applied("привет 你好 é");
        e.key(K_BACK);
        e.key(K_BACK);
        assert_eq!(e.draft(), "привет 你好");
        e.key(K_BACK);
        assert_eq!(e.draft(), "привет 你");
    }

    #[test]
    fn state_round_trips_and_fingerprint_follows_the_screen() {
        let mut e = Editor::default();
        typed(&mut e, "sing");
        e.key(K_DONE);
        typed(&mut e, " it, привет");
        e.key(K_LEFT);
        e.key(K_SHIFT);
        let back = Editor::from_bytes(&e.to_bytes()).unwrap();
        assert_eq!((back.applied(), back.draft(), back.lines(30, 3), back.status()), (e.applied(), e.draft(), e.lines(30, 3), e.status()));
        assert_eq!(back.fingerprint(), e.fingerprint());
        let f = e.fingerprint();
        e.key(K_RIGHT);
        assert_ne!(e.fingerprint(), f);
        assert!(Editor::from_bytes(b"junk").is_none());
    }

    #[test]
    fn long_lyrics_are_capped_wrapped_and_every_line_fits_the_host() {
        let mut e = Editor::default();
        let verse = "Hello there, I am a voice made of air. I sing whatever words you type, line by line, \
                     until the very last syllable of this rather long second sentence.";
        typed(&mut e, verse);
        assert_eq!(e.draft(), verse);
        let fits = |e: &Editor| {
            let l = e.lines(LINE_BYTES, ROWS);
            assert_eq!(l.len(), ROWS);
            for x in &l {
                assert!(x.len() <= LINE_BYTES, "{x:?} is {} bytes", x.len());
            }
            assert!(e.status().len() <= LINE_BYTES);
            l
        };
        // typing at the end: the cursor's line is the last one shown, the text before above it
        let l = fits(&e);
        assert!(l[2].ends_with("sentence.|"), "{l:?}");
        assert!(!l[0].is_empty() && !l[1].is_empty(), "{l:?}");
        // all of it can be walked through: the cursor is always on screen
        for _ in 0..verse.len() {
            e.key(K_LEFT);
            assert!(fits(&e).iter().any(|x| x.contains('|')));
        }
        assert!(fits(&e)[0].starts_with("|Hello there"));
        // a word longer than a line is cut, not lost
        let mut w = Editor::default();
        typed(&mut w, &"x".repeat(2 * LINE_BYTES + 5));
        let l = fits(&w);
        assert_eq!(l.concat().matches('x').count(), 2 * LINE_BYTES + 5, "{l:?}");
        // the cap
        for _ in 0..MAX_CHARS + 50 {
            e.key(b'a');
        }
        assert_eq!(e.draft().chars().count(), MAX_CHARS);
        fits(&e);
        // non-ASCII lines are cut by bytes, on character boundaries
        let mut u = Editor::default();
        u.set_applied(&"привет ".repeat(12));
        fits(&u);
    }
}
