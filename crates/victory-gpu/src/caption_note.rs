//! A short note the window caption shows for a few seconds after the frame
//! rate (the keyboard's driving mode, `victory-hle`'s `gameinput`). The
//! caption is written about once a second, so a note shows within a second
//! and stays [`SHOWN`].

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a note stays in the caption.
pub const SHOWN: Duration = Duration::from_secs(4);

static NOTE: Mutex<Option<(String, Instant)>> = Mutex::new(None);

/// Shows `text` in the caption for [`SHOWN`], in place of any earlier note.
pub fn show(text: impl Into<String>) {
    *NOTE.lock().unwrap_or_else(|e| e.into_inner()) = Some((text.into(), Instant::now()));
}

/// The note to show now, if one is still due.
pub fn current() -> Option<String> {
    let mut note = NOTE.lock().unwrap_or_else(|e| e.into_inner());
    if note.as_ref().is_some_and(|(_, at)| at.elapsed() >= SHOWN) {
        *note = None;
    }
    note.as_ref().map(|(text, _)| text.clone())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_note_shows_until_replaced() {
        super::show("[driving]");
        assert_eq!(super::current().as_deref(), Some("[driving]"));
        super::show("[on foot]");
        assert_eq!(super::current().as_deref(), Some("[on foot]"));
    }
}
