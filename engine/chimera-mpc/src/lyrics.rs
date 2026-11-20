//! lyrics.txt: the words the instrument sings, read from the MPC's own storage (no computer).
//!
//! The file holds one or more lyric sets. A line starting with `#` begins a set and names it;
//! text before the first `#` line (or the whole file, with no `#` at all) is a set of its own.
//! Inside a set, line breaks and punctuation are phrase breaks, as in the web app's lyrics box, and
//! a `-` on its own holds the previous vowel onto the next note (melisma). Lines starting with
//! `//` are comments.
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct LyricSet {
    pub title: String,
    pub text: String,
}

/// Split a lyrics file into its sets (empty sets are dropped).
pub fn parse(file: &str) -> Vec<LyricSet> {
    let file = file.strip_prefix('\u{FEFF}').unwrap_or(file);
    let mut sets: Vec<LyricSet> = Vec::new();
    let mut cur = LyricSet { title: String::new(), text: String::new() };
    let flush = |cur: &mut LyricSet, sets: &mut Vec<LyricSet>| {
        let text = cur.text.trim().to_string();
        if !text.is_empty() {
            let title = if cur.title.is_empty() { first_words(&text) } else { cur.title.clone() };
            sets.push(LyricSet { title, text });
        }
        cur.text.clear();
    };
    for line in file.lines() {
        let l = line.trim_end_matches('\r');
        if l.trim_start().starts_with("//") {
            continue;
        }
        if let Some(t) = l.trim_start().strip_prefix('#') {
            flush(&mut cur, &mut sets);
            cur.title = t.trim().to_string();
            continue;
        }
        cur.text.push_str(l);
        cur.text.push('\n');
    }
    flush(&mut cur, &mut sets);
    sets
}

/// A title for an untitled set: its first few words.
fn first_words(text: &str) -> String {
    let mut s = String::new();
    for w in text.split_whitespace() {
        if s.chars().count() + w.chars().count() > 14 {
            break;
        }
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(w);
    }
    if s.is_empty() { text.chars().take(14).collect() } else { s }
}

/// Where lyrics.txt is looked for, first found wins: the drive's root (the folder holding
/// `VST2 Plugins`), the plugin's own folder, then its `Content/` folder (where the shipped
/// example lives).
pub fn candidates(plugin_dir: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(vst2) = plugin_dir.parent() {
        if let Some(drive) = vst2.parent() {
            v.push(drive.join("lyrics.txt"));
            v.push(drive.join("Chimera Voice").join("lyrics.txt"));
        }
    }
    v.push(plugin_dir.join("lyrics.txt"));
    v.push(plugin_dir.join("Content").join("lyrics.txt"));
    v
}

/// The first lyrics file that exists, with its modification stamp.
pub fn find(plugin_dir: &Path) -> Option<(PathBuf, std::time::SystemTime)> {
    candidates(plugin_dir).into_iter().find_map(|p| {
        let m = std::fs::metadata(&p).ok()?;
        if !m.is_file() {
            return None;
        }
        Some((p, m.modified().unwrap_or(std::time::UNIX_EPOCH)))
    })
}

/// Read a text file, lossily (a file saved in a legacy encoding still loads).
pub fn read_text(p: &Path) -> Option<String> {
    let bytes = std::fs::read(p).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Read a gzip-compressed text file (the pronouncing dictionary, the Mandarin lexicon).
pub fn read_gz(p: &Path) -> Option<String> {
    let f = std::fs::File::open(p).ok()?;
    let mut s = String::new();
    flate2::read::GzDecoder::new(std::io::BufReader::new(f)).read_to_string(&mut s).ok()?;
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_and_titles() {
        let f = "\u{FEFF}// a comment\nfirst words before any title\n\n# Song A\nla la\nla\n#Song B\n\n# Empty\n";
        let s = parse(f);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].title, "first words");
        assert_eq!(s[1].title, "Song A");
        assert_eq!(s[1].text, "la la\nla");
    }

    #[test]
    fn whole_file_is_one_set() {
        let s = parse("Hello there, I am a voice made of air.\r\n");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "Hello there, I am a voice made of air.");
    }

    #[test]
    fn drive_root_first() {
        let c = candidates(Path::new("/media/SD/VST2 Plugins/Gator Audio - VST - Chimera Voice"));
        assert_eq!(c[0], Path::new("/media/SD/lyrics.txt"));
        assert_eq!(c.last().unwrap(), &Path::new("/media/SD/VST2 Plugins/Gator Audio - VST - Chimera Voice/Content/lyrics.txt"));
    }
}
