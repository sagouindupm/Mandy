//! Texts for the window as a key and parameters, never as sentences: the window translates them
//! (launcher/src/i18n, English or French). A command's error is a `Msg` as JSON, read back by errorText there;
//! other errors (the system's own, developer ones) stay plain English text.
//!
//! A parameter is text, another message, or a list of messages (joined by the window). A message built from a
//! text of series-s/reglages.json carries it as the parameter `text`: the window shows it in French, or its
//! English translation keyed by `key`.

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum Param {
    Text(String),
    Msg(Msg),
    List(Vec<Msg>),
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Msg {
    pub key: String,
    pub params: BTreeMap<String, Param>,
}

impl Msg {
    pub fn new(key: &str) -> Self {
        Msg { key: key.to_string(), params: BTreeMap::new() }
    }

    /// A text of series-s/reglages.json (French), translated by the window from `key`.
    pub fn data(key: &str, text: &str) -> Self {
        Msg::new(key).with("text", text)
    }

    pub fn with(mut self, k: &str, v: impl Into<Param>) -> Self {
        self.params.insert(k.to_string(), v.into());
        self
    }

    /// The `text` parameter if there is one (reglages.json), else the message as `Display` gives it.
    pub fn plain(&self) -> String {
        match self.params.get("text") {
            Some(Param::Text(t)) => t.clone(),
            _ => self.to_string(),
        }
    }
}

/// As a command's error: JSON, translated by the window.
impl From<Msg> for String {
    fn from(m: Msg) -> String {
        serde_json::to_string(&m).unwrap_or(m.key)
    }
}

impl From<String> for Param {
    fn from(v: String) -> Self {
        Param::Text(v)
    }
}
impl From<&str> for Param {
    fn from(v: &str) -> Self {
        Param::Text(v.to_string())
    }
}
impl From<Msg> for Param {
    fn from(v: Msg) -> Self {
        Param::Msg(v)
    }
}
impl From<Vec<Msg>> for Param {
    fn from(v: Vec<Msg>) -> Self {
        Param::List(v)
    }
}
macro_rules! number_param {
    ($($t:ty),*) => {$(
        impl From<$t> for Param {
            fn from(v: $t) -> Self {
                Param::Text(v.to_string())
            }
        }
    )*};
}
number_param!(usize, u64, i64, i32, f64);

/// For developers and tests: `key(name=value, ...)`, nested messages the same way, lists joined by « ; ».
impl fmt::Display for Msg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.key)?;
        if !self.params.is_empty() {
            let parts: Vec<String> = self.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
            write!(f, "({})", parts.join(", "))?;
        }
        Ok(())
    }
}

impl fmt::Display for Param {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Param::Text(t) => f.write_str(t),
            Param::Msg(m) => write!(f, "{m}"),
            Param::List(l) => f.write_str(&l.iter().map(|m| m.to_string()).collect::<Vec<_>>().join("; ")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_json() {
        let m = Msg::new("game.incomplete").with("dir", "C:\\x").with("what", vec![Msg::new("game.fewArchives").with("count", 1usize).with("min", 8usize)]);
        assert_eq!(m.to_string(), "game.incomplete(dir=C:\\x, what=game.fewArchives(count=1, min=8))");
        let json: String = m.into();
        assert_eq!(json, r#"{"key":"game.incomplete","params":{"dir":"C:\\x","what":[{"key":"game.fewArchives","params":{"count":"1","min":"8"}}]}}"#);
        assert_eq!(Msg::data("jeu.conseil", "Gardez").plain(), "Gardez");
    }
}
