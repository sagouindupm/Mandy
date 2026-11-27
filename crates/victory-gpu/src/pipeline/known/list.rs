//! Shared pipeline lists: the pipelines a book holds (`known-pipelines.bin`),
//! without a byte of the title's shader code, so that a player can make them
//! all (`victory warm`) before a first game, with the shaders of their own
//! copy of the title.
//!
//! A list names each shader by the BLAKE3 of its container, as the book does;
//! the containers themselves are taken from the player's own files
//! ([`Keeper`]: `victory shaders` and `victory pipelines import --shaders`
//! put the ones the book lacks into its `known-shaders.bin`).
//!
//! # The format (version 1)
//!
//! Plain text (the repository holds text only, `docs/asset-policy.md`), one
//! record a line, fields split by one space:
//!
//! ```text
//! victory-pipelines 1
//! keys 1
//! states 1
//! label <printable ASCII, may be empty>
//! only-used <1: only the pipelines earlier runs drew with; 0: every one booked>
//! # <a comment: any printable ASCII; anywhere before `end`>
//! state <depth bounds> <2 + N numbers> [<name>/<index>/<format>/<slot>/<offset>/<class>/<step rate> ...]
//! shader <stage> <BLAKE3 of the container: 64 hex digits>
//! pipeline <key: 32 hex digits> <u: drawn with | -> <its shaders' numbers, a comma between> <its state's number | ->
//! end <BLAKE3 of every line before this one, each ended by a line feed: 64 hex digits>
//! ```
//!
//! - `keys 1`: a key is the first 16 bytes of the BLAKE3 of each shader's
//!   stage name and the first 16 hex digits of its hash, then the graphics
//!   state's description, as a run makes it (`make::Pending`). An import
//!   makes each key again from the shaders and state, as the importing build
//!   makes keys, and books the pipeline under that key ([`Imported::rekeyed`]
//!   counts those that came out otherwise).
//! - `states 1`: a state is the graphics state as `known-pipelines.bin` holds
//!   it (`GraphicsState::put`): whether it has depth bounds, the bits of the
//!   depth-bias clamp and slope as numbers, then N numbers (blend, rasterizer,
//!   depth-stencil, topology, targets, samples), then its input elements.
//!   Each is written once, numbered from 0 in order; a compute pipeline has none.
//! - Shaders are numbered from 0 in order, each once; `<stage>` is one of
//!   vertex, pixel, domain, hull, geometry, compute, amplification, mesh.
//! - Lines come in that order: the five header lines, then states, shaders
//!   and pipelines. Carriage returns are ignored.
//!
//! Nothing else can be in a list that reads: every field is a number, a hash,
//! a key or a plain name; each state must read as a graphics state and write
//! back to the same bytes, and its input element names must be plain names
//! (letters, digits, `_`). [`List::to_text`] reads what it wrote back that way
//! before it gives it.
//!
//! What a list depends on: the title's shaders (by hash). Not the translator,
//! its settings, the driver or the card: a list made on one machine is made
//! into host pipelines on another by its own translator.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{GraphicsState, PIPELINES, SHADERS, STAGES, walk_pipelines, walk_shaders};

const HEAD: &str = "victory-pipelines";
/// The format's version; a list of another one is refused.
pub const VERSION: u32 = 1;
const KEYS: u32 = 1;
const STATES: u32 = 1;
/// Shaders in a pipeline, at most.
const MOST_SHADERS: usize = 8;
const EARLY: &str = "early-pipelines.txt";

/// A pipeline of a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pipeline {
    pub key: [u8; 16],
    /// Each shader's stage (a number of `STAGES`) and container hash.
    pub shaders: Vec<(u8, [u8; 32])>,
    /// The graphics state as `known-pipelines.bin` holds it; none for compute.
    pub state: Option<Vec<u8>>,
    /// An earlier run drew or dispatched with it (the early list).
    pub used: bool,
}

impl Pipeline {
    /// The key as the early list writes it.
    pub fn key_text(&self) -> String {
        hex(&self.key)
    }

    /// From a `known-pipelines.bin` entry.
    pub(super) fn of_entry(entry: &[u8]) -> Option<Pipeline> {
        let (key, rest) = entry.split_first_chunk::<16>()?;
        let (&count, mut rest) = rest.split_first()?;
        let mut shaders = Vec::new();
        for _ in 0..count {
            let (head, after) = rest.split_first_chunk::<33>()?;
            rest = after;
            let mut hash = [0; 32];
            hash.copy_from_slice(&head[1..]);
            shaders.push((head[0], hash));
        }
        let (&graphics, rest) = rest.split_first()?;
        let state = (graphics != 0).then(|| rest.to_vec());
        Some(Pipeline {
            key: *key,
            shaders,
            state,
            used: false,
        })
    }

    /// As a `known-pipelines.bin` entry.
    fn entry(&self) -> Vec<u8> {
        let mut entry = self.key.to_vec();
        entry.push(self.shaders.len() as u8);
        for (stage, hash) in &self.shaders {
            entry.push(*stage);
            entry.extend(hash);
        }
        entry.push(u8::from(self.state.is_some()));
        if let Some(state) = &self.state {
            entry.extend(state);
        }
        entry
    }

    /// The key this build makes for its shaders and state (`make::Pending`).
    pub fn made_key(&self) -> Option<[u8; 16]> {
        let mut h = blake3::Hasher::new();
        for (stage, hash) in &self.shaders {
            h.update(STAGES.get(usize::from(*stage))?.as_bytes());
            h.update(hex(&hash[..8]).as_bytes());
        }
        if let Some(state) = &self.state {
            h.update(GraphicsState::get(state)?.describe().as_bytes());
        }
        h.finalize().as_bytes().first_chunk::<16>().copied()
    }
}

/// A list of pipelines (see the module's documentation).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct List {
    /// Free text: where the list comes from.
    pub label: String,
    /// Only the pipelines runs drew with.
    pub only_used: bool,
    pub pipelines: Vec<Pipeline>,
}

impl List {
    /// The pipelines booked in the cache folder `dir` (`known-pipelines.bin`),
    /// marked used where the early list has them; with `only_used`, those
    /// alone. Never reads `known-shaders.bin`.
    pub fn of_book(dir: &Path, only_used: bool, label: &str) -> List {
        let early = early_keys(dir);
        let mut seen = HashSet::new();
        let mut pipelines = Vec::new();
        walk_pipelines(dir, |_, entry| {
            if let Some(mut p) = Pipeline::of_entry(entry)
                && seen.insert(p.key)
            {
                p.used = early.contains(&p.key_text());
                if p.used || !only_used {
                    pipelines.push(p);
                }
            }
        });
        let mut list = List {
            label: label.to_owned(),
            only_used,
            pipelines,
        };
        list.sort();
        list
    }

    fn sort(&mut self) {
        self.pipelines.sort_by_key(|p| p.key);
        self.pipelines.dedup_by_key(|p| p.key);
    }

    /// Every pipeline of `lists`, each once (used if one of them says so).
    pub fn merge<'a>(lists: impl IntoIterator<Item = &'a List>, label: &str) -> List {
        let mut all: BTreeMap<[u8; 16], Pipeline> = BTreeMap::new();
        let mut only_used = true;
        for list in lists {
            only_used &= list.only_used;
            for p in &list.pipelines {
                all.entry(p.key)
                    .and_modify(|q| q.used |= p.used)
                    .or_insert_with(|| p.clone());
            }
        }
        List {
            label: label.to_owned(),
            only_used,
            pipelines: all.into_values().collect(),
        }
    }

    /// Pipelines of the list runs drew with.
    pub fn used(&self) -> usize {
        self.pipelines.iter().filter(|p| p.used).count()
    }

    /// Pipelines whose key this build makes otherwise.
    pub fn rekeyed(&self) -> usize {
        self.pipelines
            .iter()
            .filter(|p| p.made_key() != Some(p.key))
            .count()
    }

    /// The list in the format; read back and checked before it is given
    /// (an error if it would hold anything but the format's fields).
    pub fn to_text(&self) -> Result<String, String> {
        if !plain_text(&self.label) {
            return Err("the label must be at most 200 printable ASCII characters".into());
        }
        let mut list = self.clone();
        list.sort();
        let mut states: BTreeMap<&[u8], usize> = BTreeMap::new();
        let mut shaders: BTreeMap<[u8; 32], (u8, usize)> = BTreeMap::new();
        for p in &list.pipelines {
            if p.shaders.is_empty() || p.shaders.len() > MOST_SHADERS {
                return Err(format!(
                    "pipeline {} has {} shaders",
                    p.key_text(),
                    p.shaders.len()
                ));
            }
            if let Some(state) = &p.state {
                states.insert(state, 0);
            }
            for &(stage, hash) in &p.shaders {
                if usize::from(stage) >= STAGES.len() {
                    return Err(format!("stage {stage}"));
                }
                shaders.insert(hash, (stage, 0));
            }
        }
        for (n, place) in states.values_mut().enumerate() {
            *place = n;
        }
        for (n, (_, place)) in shaders.values_mut().enumerate() {
            *place = n;
        }
        let mut out = format!(
            "{HEAD} {VERSION}\nkeys {KEYS}\nstates {STATES}\nlabel {}\nonly-used {}\n",
            list.label,
            u8::from(list.only_used)
        );
        let _ = writeln!(
            out,
            "# {} pipelines ({} drawn with), {} graphics states, {} shaders by the BLAKE3 of their container: no shader code",
            list.pipelines.len(),
            list.used(),
            states.len(),
            shaders.len()
        );
        for state in states.keys() {
            out.push_str("state");
            state_text(state, &mut out).ok_or("a graphics state that does not read")?;
            out.push('\n');
        }
        for (hash, (stage, _)) in &shaders {
            let _ = writeln!(out, "shader {} {}", STAGES[usize::from(*stage)], hex(hash));
        }
        for p in &list.pipelines {
            let places: Vec<String> = p
                .shaders
                .iter()
                .map(|(_, hash)| shaders[hash].1.to_string())
                .collect();
            let state = p
                .state
                .as_ref()
                .map_or("-".to_owned(), |s| states[s.as_slice()].to_string());
            let used = if p.used { "u" } else { "-" };
            let _ = writeln!(
                out,
                "pipeline {} {used} {} {state}",
                p.key_text(),
                places.join(",")
            );
        }
        let _ = writeln!(out, "end {}", blake3::hash(out.as_bytes()).to_hex());
        // What is written is what reads back, and nothing else.
        if List::from_text(&out)? != list {
            return Err("the list does not read back as it was".into());
        }
        Ok(out)
    }

    /// A list from its text, checked (see the module's documentation).
    pub fn from_text(text: &str) -> Result<List, String> {
        let text = text.replace('\r', "");
        let mut body = String::new();
        let mut lines = text.lines().enumerate();
        let mut next = |what: &str| -> Result<(usize, String), String> {
            loop {
                let (n, line) = lines
                    .next()
                    .ok_or_else(|| format!("the list ends before {what}"))?;
                if !line.starts_with("end ") {
                    body.push_str(line);
                    body.push('\n');
                }
                if !line.starts_with('#') {
                    return Ok((n + 1, line.to_owned()));
                }
                if !line.bytes().all(|b| (0x20..0x7f).contains(&b)) {
                    return Err(format!("line {}: a comment that is not plain text", n + 1));
                }
            }
        };
        let header = |line: &str, name: &str| -> Result<String, String> {
            line.strip_prefix(name)
                .and_then(|rest| rest.strip_prefix(' ').or((rest.is_empty()).then_some("")))
                .map(str::to_owned)
                .ok_or_else(|| format!("`{name}` expected, not {line:?}"))
        };
        let (_, line) = next("the head")?;
        let version = header(&line, HEAD).map_err(|_| "not a pipeline list".to_owned())?;
        if version != VERSION.to_string() {
            return Err(format!(
                "a list of format {version}; this build reads {VERSION}"
            ));
        }
        for (name, value) in [("keys", KEYS), ("states", STATES)] {
            let (_, line) = next(name)?;
            if header(&line, name)? != value.to_string() {
                return Err(format!("{line:?}: this build reads `{name} {value}`"));
            }
        }
        let (_, line) = next("label")?;
        let label = header(&line, "label")?;
        if !plain_text(&label) {
            return Err("the label is not plain text".into());
        }
        let (_, line) = next("only-used")?;
        let only_used = match header(&line, "only-used")?.as_str() {
            "0" => false,
            "1" => true,
            other => return Err(format!("only-used {other:?}")),
        };
        let mut states: Vec<Vec<u8>> = Vec::new();
        let mut shaders: Vec<(u8, [u8; 32])> = Vec::new();
        let mut pipelines = Vec::new();
        // Which kind of line came last: states, shaders, pipelines.
        let mut part = 0;
        let sum = loop {
            let (n, line) = next("end")?;
            let at = |e: String| format!("line {n}: {e}");
            let (kind, rest) = line.split_once(' ').unwrap_or((&line, ""));
            let this = match kind {
                "state" => 0,
                "shader" => 1,
                "pipeline" => 2,
                "end" => break rest.to_owned(),
                _ => return Err(at(format!("{kind:?} is not a line of a list"))),
            };
            if this < part {
                return Err(at(format!(
                    "a {kind} line after the {}",
                    ["states", "shaders", "pipelines"][part]
                )));
            }
            part = this;
            match this {
                0 => states.push(state_bytes(rest).map_err(at)?),
                1 => {
                    let (stage, hash) = rest.split_once(' ').ok_or_else(|| at("shader".into()))?;
                    let stage = STAGES
                        .iter()
                        .position(|&s| s == stage)
                        .ok_or_else(|| at(format!("stage {stage:?}")))?;
                    let hash =
                        unhex::<32>(hash).ok_or_else(|| at("a hash of 64 hex digits".into()))?;
                    shaders.push((stage as u8, hash));
                }
                _ => {
                    let fields: Vec<&str> = rest.split(' ').collect();
                    let [key, used, its, state] = fields[..] else {
                        return Err(at("a pipeline has four fields".into()));
                    };
                    let key =
                        unhex::<16>(key).ok_or_else(|| at("a key of 32 hex digits".into()))?;
                    let used = match used {
                        "u" => true,
                        "-" => false,
                        _ => return Err(at(format!("{used:?}: `u` or `-`"))),
                    };
                    let its: Vec<(u8, [u8; 32])> = its
                        .split(',')
                        .map(|n| {
                            n.parse::<usize>()
                                .ok()
                                .and_then(|n| shaders.get(n).copied())
                        })
                        .collect::<Option<_>>()
                        .ok_or_else(|| at("a shader the list does not have".into()))?;
                    if its.is_empty() || its.len() > MOST_SHADERS {
                        return Err(at(format!("a pipeline of {} shaders", its.len())));
                    }
                    let state = match state {
                        "-" => None,
                        n => Some(
                            n.parse::<usize>()
                                .ok()
                                .and_then(|n| states.get(n).cloned())
                                .ok_or_else(|| at("a state the list does not have".into()))?,
                        ),
                    };
                    pipelines.push(Pipeline {
                        key,
                        shaders: its,
                        state,
                        used,
                    });
                }
            }
        };
        if lines.next().is_some() {
            return Err("lines after `end`".into());
        }
        if unhex::<32>(&sum) != Some(*blake3::hash(body.as_bytes()).as_bytes()) {
            return Err("the list is damaged: its checksum does not match".into());
        }
        Ok(List {
            label,
            only_used,
            pipelines,
        })
    }
}

/// Numbers in a state between its depth bounds and its input elements: the
/// two of the depth bias, then those of `GraphicsState::put`.
fn state_words() -> usize {
    let mut bytes = Vec::new();
    GraphicsState::default().put(&mut bytes);
    // Depth bounds, and the count of input elements: a byte each.
    (bytes.len() - 2) / 4
}

/// Appends ` <field>...` of a state's bytes to `out` (checked first).
fn state_text(bytes: &[u8], out: &mut String) -> Option<()> {
    checked_state(bytes).ok()?;
    let words = state_words();
    let (&bounds, mut rest) = bytes.split_first()?;
    let _ = write!(out, " {bounds}");
    let word = |rest: &mut &[u8]| -> Option<u32> {
        let (head, after) = rest.split_first_chunk::<4>()?;
        *rest = after;
        Some(u32::from_le_bytes(*head))
    };
    for _ in 0..words {
        let _ = write!(out, " {}", word(&mut rest)?);
    }
    let (&count, after) = rest.split_first()?;
    rest = after;
    for _ in 0..count {
        let (&len, after) = rest.split_first()?;
        let (name, after) = after.split_at_checked(usize::from(len))?;
        rest = after;
        let _ = write!(out, " {}", std::str::from_utf8(name).ok()?);
        for _ in 0..6 {
            let _ = write!(out, "/{}", word(&mut rest)?);
        }
    }
    rest.is_empty().then_some(())
}

/// A state's bytes from its fields (checked).
fn state_bytes(text: &str) -> Result<Vec<u8>, String> {
    let words = state_words();
    let fields: Vec<&str> = text.split(' ').collect();
    if fields.len() < 1 + words {
        return Err(format!("a state of {} fields", fields.len()));
    }
    let number = |f: &str| {
        f.parse::<u32>()
            .map_err(|_| format!("{f:?} is not a number"))
    };
    let bounds = u8::try_from(number(fields[0])?).map_err(|_| "depth bounds")?;
    let mut bytes = vec![bounds];
    for f in &fields[1..1 + words] {
        bytes.extend(number(f)?.to_le_bytes());
    }
    let elements = &fields[1 + words..];
    bytes.push(u8::try_from(elements.len()).map_err(|_| "too many input elements")?);
    for e in elements {
        let parts: Vec<&str> = e.split('/').collect();
        let [name, numbers @ ..] = &parts[..] else {
            return Err(format!("input element {e:?}"));
        };
        if numbers.len() != 6 || name.len() > 64 {
            return Err(format!("input element {e:?}"));
        }
        bytes.push(name.len() as u8);
        bytes.extend(name.as_bytes());
        for n in numbers {
            bytes.extend(number(n)?.to_le_bytes());
        }
    }
    checked_state(&bytes)?;
    Ok(bytes)
}

/// A state reads as a graphics state, writes back to the same bytes, and
/// its only text (semantic names) is short plain names.
fn checked_state(bytes: &[u8]) -> Result<(), String> {
    let g = GraphicsState::get(bytes).ok_or("a graphics state that does not read")?;
    let mut back = Vec::new();
    g.put(&mut back);
    if back != bytes {
        return Err("a graphics state with bytes of its own".into());
    }
    for name in &g.names {
        let name = name.to_bytes();
        if name.is_empty()
            || name.len() > 64
            || !name.iter().all(|&b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("an input element name that is not a plain name".into());
        }
    }
    Ok(())
}

fn plain_text(text: &str) -> bool {
    text.len() <= 200 && text.bytes().all(|b| (0x20..0x7f).contains(&b))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != 2 * N {
        return None;
    }
    let mut out = [0; N];
    for (at, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(text.get(2 * at..2 * at + 2)?, 16).ok()?;
    }
    Some(out)
}

fn early_keys(dir: &Path) -> HashSet<String> {
    let text = std::fs::read_to_string(dir.join(EARLY)).unwrap_or_default();
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// What an import did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Imported {
    /// Pipelines added to the book, and those it had already.
    pub added: usize,
    pub there: usize,
    /// Pipelines whose key this build makes otherwise (another build's
    /// list): booked under this build's key.
    pub rekeyed: usize,
    /// Keys added to the early list.
    pub early: usize,
}

/// Adds the pipelines of `list` the book in `dir` lacks to it, and those runs
/// drew with to the early list, under the keys this build makes. Not while a
/// run books pipelines in `dir`.
pub fn import(dir: &Path, list: &List) -> std::io::Result<Imported> {
    std::fs::create_dir_all(dir)?;
    let mut keys = HashSet::new();
    let end = walk_pipelines(dir, |_, entry| {
        keys.extend(entry.first_chunk::<16>().copied());
    });
    let mut done = Imported::default();
    let mut new = Vec::new();
    let mut early = Vec::new();
    for p in &list.pipelines {
        let mut p = p.clone();
        // (A list that reads has states that read.)
        let Some(key) = p.made_key() else { continue };
        if key != p.key {
            done.rekeyed += 1;
            p.key = key;
        }
        if p.used {
            early.push(p.key_text());
        }
        if !keys.insert(p.key) {
            done.there += 1;
            continue;
        }
        let entry = p.entry();
        new.extend((entry.len() as u32).to_le_bytes());
        new.extend(entry);
        done.added += 1;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(dir.join(PIPELINES))?;
    // A torn end is cut off first, as a run does.
    file.set_len(end)?;
    std::io::Seek::seek(&mut file, std::io::SeekFrom::End(0))?;
    file.write_all(&new)?;
    drop(file);
    let mut listed: Vec<String> = std::fs::read_to_string(dir.join(EARLY))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    let mut have: HashSet<String> = listed.iter().cloned().collect();
    for key in early {
        if have.insert(key.clone()) {
            listed.push(key);
            done.early += 1;
        }
    }
    if done.early > 0 {
        let tmp = dir.join(format!("{EARLY}.tmp-{}", std::process::id()));
        std::fs::write(&tmp, listed.join("\n"))?;
        std::fs::rename(&tmp, dir.join(EARLY))?;
    }
    Ok(done)
}

/// Puts the containers that the book's pipelines name and its
/// `known-shaders.bin` lacks into that file, from the player's own files
/// as they are walked (`victory shaders`, `victory pipelines import
/// --shaders`): what `victory warm` makes host pipelines from.
pub struct Keeper {
    /// Hash and stage of each container wanted.
    wanted: Mutex<HashMap<[u8; 32], u8>>,
    file: Mutex<std::fs::File>,
    kept: AtomicUsize,
}

impl Keeper {
    /// For the book in `dir`; `None` when it lacks no container.
    pub fn of(dir: &Path) -> std::io::Result<Option<Keeper>> {
        let mut wanted = HashMap::new();
        walk_pipelines(dir, |_, entry| {
            if let Some(p) = Pipeline::of_entry(entry) {
                wanted.extend(p.shaders.iter().map(|&(stage, hash)| (hash, stage)));
            }
        });
        let end = walk_shaders(dir, |_, hash, _, _| {
            wanted.remove(&hash);
        });
        if wanted.is_empty() {
            return Ok(None);
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(dir.join(SHADERS))?;
        file.set_len(end)?;
        std::io::Seek::seek(&mut file, std::io::SeekFrom::End(0))?;
        Ok(Some(Keeper {
            wanted: Mutex::new(wanted),
            file: Mutex::new(file),
            kept: AtomicUsize::new(0),
        }))
    }

    /// Containers still wanted.
    pub fn wanted(&self) -> usize {
        self.wanted.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Containers kept so far.
    pub fn kept(&self) -> usize {
        self.kept.load(Ordering::Relaxed)
    }

    /// A container of the player's files, and its BLAKE3: kept if wanted.
    pub fn offer(&self, hash: &[u8; 32], container: &[u8]) -> std::io::Result<bool> {
        let Some(stage) = self
            .wanted
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(hash)
        else {
            return Ok(false);
        };
        let mut record = Vec::with_capacity(37 + container.len());
        record.push(stage);
        record.extend(hash);
        record.extend((container.len() as u32).to_le_bytes());
        record.extend(container);
        self.file
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .write_all(&record)?;
        self.kept.fetch_add(1, Ordering::Relaxed);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use windows::Win32::Graphics::Direct3D12::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::core::PCSTR;

    use super::*;

    /// What the book's containers hold: the title's own code, never shared.
    const CODE: &[u8] = b"DXBC-the-title's-own-code-never-shared";

    fn state(n: u32) -> Vec<u8> {
        let mut g = GraphicsState {
            sample_mask: n,
            depth_bounds: n == 1,
            ..Default::default()
        };
        g.rasterizer.SlopeScaledDepthBias = 1.5;
        g.targets.NumRenderTargets = 1;
        g.targets.RTFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
        g.names = vec![
            CString::new("POSITION").unwrap(),
            CString::new("TEXCOORD").unwrap(),
        ];
        g.inputs = g
            .names
            .iter()
            .map(|name| D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(name.as_ptr().cast()),
                Format: DXGI_FORMAT_R32G32B32_FLOAT,
                AlignedByteOffset: D3D12_APPEND_ALIGNED_ELEMENT,
                ..Default::default()
            })
            .collect();
        let mut bytes = Vec::new();
        g.put(&mut bytes);
        bytes
    }

    fn pipeline(n: u8, graphics: bool) -> Pipeline {
        let shaders = if graphics {
            vec![(0, [n; 32]), (1, [n + 100; 32])]
        } else {
            vec![(5, [n; 32])]
        };
        let mut p = Pipeline {
            key: [0; 16],
            shaders,
            state: graphics.then(|| state(u32::from(n % 3))),
            used: n.is_multiple_of(2),
        };
        p.key = p.made_key().unwrap();
        p
    }

    /// A book as a run writes it: shader records whose containers hold
    /// `CODE`, pipeline entries, and the early list.
    fn book(dir: &Path, pipelines: &[Pipeline]) {
        let mut shaders = Vec::new();
        let mut entries = Vec::new();
        for p in pipelines {
            for (stage, hash) in &p.shaders {
                let container = [CODE, hash].concat();
                shaders.push(*stage);
                shaders.extend(hash);
                shaders.extend((container.len() as u32).to_le_bytes());
                shaders.extend(container);
            }
            let entry = p.entry();
            entries.extend((entry.len() as u32).to_le_bytes());
            entries.extend(entry);
        }
        std::fs::write(dir.join(SHADERS), shaders).unwrap();
        std::fs::write(dir.join(PIPELINES), entries).unwrap();
        let early: Vec<String> = pipelines
            .iter()
            .filter(|p| p.used)
            .map(Pipeline::key_text)
            .collect();
        std::fs::write(dir.join(EARLY), early.join("\n")).unwrap();
    }

    #[test]
    fn a_list_comes_back_as_it_was_and_holds_no_code() {
        let dir = tempfile::tempdir().unwrap();
        let all: Vec<Pipeline> = (0..6).map(|n| pipeline(n, n != 5)).collect();
        book(dir.path(), &all);
        let list = List::of_book(dir.path(), false, "test");
        assert_eq!(list.pipelines.len(), 6);
        assert_eq!(list.used(), 3);
        assert_eq!(list.rekeyed(), 0);
        let text = list.to_text().unwrap();
        assert_eq!(List::from_text(&text).unwrap(), list);
        // Line ends of another system read the same.
        assert_eq!(List::from_text(&text.replace('\n', "\r\n")).unwrap(), list);
        // The containers' bytes are nowhere in it: text of the format's fields only.
        assert!(!text.contains("DXBC"));
        assert!(
            text.bytes()
                .all(|b| b == b'\n' || (0x20..0x7f).contains(&b))
        );
        // Each state once.
        assert_eq!(text.lines().filter(|l| l.starts_with("state ")).count(), 3);
        let used = List::of_book(dir.path(), true, "test");
        assert_eq!(used.pipelines.len(), 3);
        assert!(used.pipelines.iter().all(|p| p.used) && used.only_used);
    }

    #[test]
    fn damaged_or_padded_lists_are_refused() {
        let list = List {
            label: "x".into(),
            only_used: false,
            pipelines: vec![pipeline(1, true), pipeline(2, false)],
        };
        let text = list.to_text().unwrap();
        // A changed field, a cut end, a line after the end.
        let changed = text.replacen("pipeline ", "pipeline f", 1);
        assert!(List::from_text(&changed).is_err());
        let cut = &text[..text.rfind("end ").unwrap()];
        assert!(List::from_text(cut).is_err());
        assert!(List::from_text(&format!("{text}state 0\n")).is_err());
        // A changed field under a checksum made again is still checked.
        let resum = |text: &str| {
            let body = &text[..text.rfind("end ").unwrap()];
            format!("{body}end {}\n", blake3::hash(body.as_bytes()).to_hex())
        };
        assert_eq!(List::from_text(&resum(&text)).unwrap(), list);
        assert!(List::from_text(&resum(&text.replacen("POSITION/", "POS\u{1}ITION/", 1))).is_err());
        assert!(
            List::from_text(&resum(&text.replacen("POSITION/0/", "POSITION/0/2/", 1))).is_err()
        );
        assert!(
            List::from_text(&resum(&text.replacen("shader vertex", "shader vertex2", 1))).is_err()
        );
        // Bytes slipped into a state are refused when writing.
        let mut padded = list.clone();
        padded.pipelines[0].state.as_mut().unwrap().extend(CODE);
        assert!(padded.to_text().is_err());
        assert!(
            List {
                label: "\n".into(),
                ..list
            }
            .to_text()
            .is_err()
        );
        assert!(List::from_text("not a list").is_err());
    }

    #[test]
    fn lists_merge_by_key() {
        let a = List {
            label: "a".into(),
            only_used: true,
            pipelines: vec![pipeline(1, true), pipeline(2, true)],
        };
        let mut b = List {
            label: "b".into(),
            only_used: false,
            pipelines: vec![pipeline(2, true), pipeline(3, false)],
        };
        b.pipelines[0].used = true;
        let merged = List::merge([&a, &b], "ab");
        assert_eq!(merged.pipelines.len(), 3);
        assert!(!merged.only_used);
        assert!(
            merged
                .pipelines
                .iter()
                .find(|p| p.key == b.pipelines[0].key)
                .unwrap()
                .used
        );
        let back = List::from_text(&merged.to_text().unwrap()).unwrap();
        assert_eq!(back, merged);
    }

    #[test]
    fn an_import_fills_the_book_and_the_shaders_come_from_the_players_files() {
        let from = tempfile::tempdir().unwrap();
        let all: Vec<Pipeline> = (0..4).map(|n| pipeline(n, n != 3)).collect();
        book(from.path(), &all);
        let mut list =
            List::from_text(&List::of_book(from.path(), false, "").to_text().unwrap()).unwrap();
        // A key this build makes otherwise is booked under its own.
        let mut other = pipeline(9, true);
        let made = other.key;
        other.key[0] ^= 1;
        list.pipelines.push(other);
        let to = tempfile::tempdir().unwrap();
        book(to.path(), &all[..1]);
        let done = import(to.path(), &list).unwrap();
        assert_eq!(
            done,
            Imported {
                added: 4,
                there: 1,
                rekeyed: 1,
                early: 1
            }
        );
        assert_eq!(super::super::count(to.path()), 5);
        assert!(super::super::keys(to.path()).contains(&hex(&made)));
        assert_eq!(early_keys(to.path()).len(), 2);
        // Again: nothing more.
        assert_eq!(import(to.path(), &list).unwrap().added, 0);
        assert_eq!(super::super::lacking(to.path()).len(), 4);
        // The player's files: every container the book lacks, and others.
        let keeper = Keeper::of(to.path()).unwrap().unwrap();
        assert_eq!(keeper.wanted(), 7);
        for p in all.iter().chain([&pipeline(9, true)]) {
            for (_, hash) in &p.shaders {
                assert!(keeper.offer(hash, &[CODE, hash].concat()).unwrap() || p.key == all[0].key);
            }
        }
        assert!(!keeper.offer(&[77; 32], b"not wanted").unwrap());
        assert_eq!((keeper.kept(), keeper.wanted()), (7, 0));
        drop(keeper);
        assert!(super::super::lacking(to.path()).is_empty());
        assert!(Keeper::of(to.path()).unwrap().is_none());
    }
}
