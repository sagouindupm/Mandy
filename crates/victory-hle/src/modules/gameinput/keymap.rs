//! The keyboard and mouse layout of the stand-in pad: what each key, mouse
//! button and wheel notch presses on the pad, on foot and in a vehicle.
//!
//! The title reads a gamepad only, and a GTA-style pad does different things
//! on foot and in a vehicle (RT fires on foot, accelerates in a car), so the
//! layout has two modes: [`Mode::Foot`] and [`Mode::Driving`]. We cannot read
//! the title's state, so the mode is the player's: the key that enters and
//! leaves vehicles (F) also flips it, and V flips it alone (for when F did
//! not get in, or the title put the player out). Each change shows in the
//! window caption for a few seconds ("[driving]", "[on foot]").
//!
//! Keys are named by their place on a US keyboard (the place is read from
//! the scan code, `key_at`), so W/A/S/D are Z/Q/S/D on an AZERTY keyboard.
//!
//! `VICTORY_KEYMAP`: unset, the PC layout ([`PC`]); `classic`, the layout of
//! before ([`CLASSIC`]: one mode, Tab = View, Q/E = LB/RB, Z/C triggers);
//! else the path of a text file in the same format as [`PC`], applied over
//! it: a key a section names replaces that key's binding in that section,
//! `none` unbinds it, a line `clear` drops the section's built-in bindings.
//! Mistakes are logged with their line and the rest still applies.

use std::collections::HashMap;
use std::sync::LazyLock;

use super::{
    A, B, DOWN, LEFT, LEFT_SHOULDER, LEFT_STICK, MENU, RIGHT, RIGHT_SHOULDER, RIGHT_STICK, UP,
    VIEW, X, Y,
};

/// A key (its virtual key at its US place, below 0x100), a mouse button
/// ([`MOUSE_BUTTON`] + its bit in the mouse reading) or a wheel notch.
pub(super) type Input = u16;
pub(super) const MOUSE_BUTTON: Input = 0x100;
pub(super) const WHEEL_UP: Input = 0x110;
pub(super) const WHEEL_DOWN: Input = 0x111;

/// The pad's analog axes, in [`Out::Axis`].
const LT: u8 = 0;
const RT: u8 = 1;
const LX: u8 = 2;
const LY: u8 = 3;
const RX: u8 = 4;
const RY: u8 = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Mode {
    #[default]
    Foot,
    Driving,
}

impl Mode {
    pub(super) fn flipped(self) -> Mode {
        match self {
            Mode::Foot => Mode::Driving,
            Mode::Driving => Mode::Foot,
        }
    }

    /// As the window caption shows it.
    pub(super) fn note(self) -> &'static str {
        match self {
            Mode::Foot => "[on foot]",
            Mode::Driving => "[driving]",
        }
    }
}

/// What an input does on the stand-in pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Out {
    Button(u32),
    /// An axis pushed all the way, up / right / pressed when `positive`.
    Axis {
        axis: u8,
        positive: bool,
    },
    /// The left stick at walk speed (`WALK_SPEED`).
    Walk,
    /// Flips [`Mode`] when pressed.
    Toggle,
}

/// The PC layout, built in. Sections: `[both]` (both modes, unless the
/// mode's own section names the key), `[foot]`, `[driving]`. Pad names:
/// a b x y menu view up down left right lb rb ls rs lt rt, left_up
/// left_down left_left left_right (left stick), right_up ... (right stick),
/// walk, toggle, none; joined by `+`. The README and the launcher's help
/// page list it: change them with it.
pub(super) const PC: &str = "
[both]
Enter = a
Backspace = b
Esc = menu
M = view
Up = up
Down = down
Left = left
Right = right
H = up
Tab = lb
WheelUp = left
WheelDown = right
I = right_up
K = right_down
J = right_left
L = right_right
F = y + toggle
V = toggle

[foot]
W = left_up
S = left_down
A = left_left
D = left_right
X = walk
Shift = a
Space = x
E = rb
Q = rb
R = b
Ctrl = ls
C = rs
MMB = rs
RMB = lt
LMB = rt

[driving]
W = rt
S = lt
A = left_left
D = left_right
Space = rb
RMB = lb
LMB = rb
E = ls
C = rs
MMB = rs
R = b
Shift = a
X = x
";

/// The layout of before the PC one (`VICTORY_KEYMAP=classic`): one mode, no
/// mouse buttons, the X key walks.
pub(super) const CLASSIC: &str = "
[both]
Esc = menu
Tab = view
Enter = a
Backspace = b
X = walk
Space = x
Y = y
Up = up
Down = down
Left = left
Right = right
Q = lb
E = rb
F = ls
H = rs
W = left_up
S = left_down
A = left_left
D = left_right
I = right_up
K = right_down
J = right_left
L = right_right
Z = lt
C = rt
";

#[derive(Clone, Debug, Default)]
pub(super) struct Keymap {
    /// Both modes, on foot, driving.
    sections: [HashMap<Input, Vec<Out>>; 3],
}

impl Keymap {
    /// A layout from `text` alone; `Err` lists its mistakes.
    pub(super) fn parse(text: &str) -> Result<Keymap, Vec<String>> {
        let mut map = Keymap::default();
        let errors = map.apply(text);
        if errors.is_empty() {
            Ok(map)
        } else {
            Err(errors)
        }
    }

    /// The PC layout.
    pub(super) fn pc() -> Keymap {
        Keymap::parse(PC).expect("the built-in layout")
    }

    /// `text` over this layout; returns its mistakes, the lines that hold
    /// none applied.
    pub(super) fn apply(&mut self, text: &str) -> Vec<String> {
        let mut errors = Vec::new();
        let mut section = 0;
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let mut fail = |what: String| errors.push(format!("line {}: {what}", n + 1));
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                match section_index(name.trim()) {
                    Some(i) => section = i,
                    None => fail(format!("unknown section [{name}] (both, foot, driving)")),
                }
                continue;
            }
            if line.eq_ignore_ascii_case("clear") {
                self.sections[section].clear();
                continue;
            }
            let Some((keys, outs)) = line.split_once('=') else {
                fail(format!("'{line}': <key> = <pad button>"));
                continue;
            };
            let outs: Result<Vec<Option<Out>>, String> =
                outs.split('+').map(|o| out_named(o.trim())).collect();
            let outs: Vec<Out> = match outs {
                Ok(outs) => outs.into_iter().flatten().collect(),
                Err(e) => {
                    fail(e);
                    continue;
                }
            };
            for key in keys.split(',') {
                match inputs_named(key.trim()) {
                    Some(inputs) => {
                        for input in inputs {
                            self.sections[section].insert(input, outs.clone());
                        }
                    }
                    None => fail(format!("unknown key '{}'", key.trim())),
                }
            }
        }
        errors
    }

    /// What `input` does in `mode`.
    pub(super) fn outs(&self, mode: Mode, input: Input) -> &[Out] {
        let own = &self.sections[mode as usize + 1];
        own.get(&input)
            .or_else(|| self.sections[0].get(&input))
            .map_or(&[], Vec::as_slice)
    }

    /// The buttons `input` presses in `mode`.
    pub(super) fn buttons(&self, mode: Mode, input: Input) -> u32 {
        self.outs(mode, input).iter().fold(0, |all, o| match o {
            Out::Button(b) => all | b,
            _ => all,
        })
    }

    /// Whether one of `held` flips the mode.
    fn toggles(&self, mode: Mode, held: &[Input]) -> bool {
        held.iter()
            .any(|&i| self.outs(mode, i).contains(&Out::Toggle))
    }

    /// The stand-in pad for the inputs `held` in `mode`.
    pub(super) fn pad(&self, mode: Mode, held: &[Input]) -> super::PadState {
        let (mut plus, mut minus, mut buttons, mut walk) = ([0f32; 6], [0f32; 6], 0, false);
        for &input in held {
            for out in self.outs(mode, input) {
                match *out {
                    Out::Button(b) => buttons |= b,
                    Out::Axis {
                        axis,
                        positive: true,
                    } => plus[usize::from(axis)] = 1.0,
                    Out::Axis {
                        axis,
                        positive: false,
                    } => minus[usize::from(axis)] = 1.0,
                    Out::Walk => walk = true,
                    Out::Toggle => {}
                }
            }
        }
        let axis = |a: u8| plus[usize::from(a)] - minus[usize::from(a)];
        let (mut left_x, mut left_y) = (axis(LX), axis(LY));
        if walk {
            // Diagonals too: the stick's length is the walk speed.
            let length = left_x.hypot(left_y);
            if length > 0.0 {
                left_x *= super::WALK_SPEED / length;
                left_y *= super::WALK_SPEED / length;
            }
        }
        super::PadState {
            buttons,
            left_trigger: plus[usize::from(LT)],
            right_trigger: plus[usize::from(RT)],
            left_x,
            left_y,
            right_x: axis(RX),
            right_y: axis(RY),
        }
    }
}

/// The layout `VICTORY_KEYMAP` asks for (see the module's documentation),
/// read once.
pub(super) fn from_env() -> &'static Keymap {
    static MAP: LazyLock<Keymap> = LazyLock::new(|| {
        let asked = std::env::var("VICTORY_KEYMAP").unwrap_or_default();
        let asked = asked.trim();
        if asked.eq_ignore_ascii_case("classic") {
            tracing::info!("keyboard: the classic layout (VICTORY_KEYMAP=classic)");
            return Keymap::parse(CLASSIC).expect("the classic layout");
        }
        let mut map = Keymap::pc();
        if !asked.is_empty() {
            match std::fs::read_to_string(asked) {
                Ok(text) => {
                    let errors = map.apply(text.trim_start_matches('\u{feff}'));
                    for e in &errors {
                        tracing::warn!("VICTORY_KEYMAP {asked}: {e}");
                    }
                    tracing::info!(
                        "keyboard: the PC layout with {asked} over it ({} mistakes)",
                        errors.len()
                    );
                }
                Err(e) => tracing::warn!("VICTORY_KEYMAP {asked}: {e}; the PC layout"),
            }
        }
        map
    });
    &MAP
}

fn section_index(name: &str) -> Option<usize> {
    match name.to_ascii_lowercase().as_str() {
        "both" | "always" | "all" => Some(0),
        "foot" | "on_foot" | "on foot" => Some(1),
        "driving" | "vehicle" | "drive" => Some(2),
        _ => None,
    }
}

/// A pad name; `None` for `none`.
fn out_named(name: &str) -> Result<Option<Out>, String> {
    let axis = |axis, positive| Ok(Some(Out::Axis { axis, positive }));
    let button = |b| Ok(Some(Out::Button(b)));
    match name.to_ascii_lowercase().as_str() {
        "a" => button(A),
        "b" => button(B),
        "x" => button(X),
        "y" => button(Y),
        "menu" | "start" => button(MENU),
        "view" | "back" | "select" => button(VIEW),
        "up" | "dpad_up" => button(UP),
        "down" | "dpad_down" => button(DOWN),
        "left" | "dpad_left" => button(LEFT),
        "right" | "dpad_right" => button(RIGHT),
        "lb" => button(LEFT_SHOULDER),
        "rb" => button(RIGHT_SHOULDER),
        "ls" | "l3" => button(LEFT_STICK),
        "rs" | "r3" => button(RIGHT_STICK),
        "lt" => axis(LT, true),
        "rt" => axis(RT, true),
        "left_up" => axis(LY, true),
        "left_down" => axis(LY, false),
        "left_left" => axis(LX, false),
        "left_right" => axis(LX, true),
        "right_up" => axis(RY, true),
        "right_down" => axis(RY, false),
        "right_left" => axis(RX, false),
        "right_right" => axis(RX, true),
        "walk" => Ok(Some(Out::Walk)),
        "toggle" => Ok(Some(Out::Toggle)),
        "none" => Ok(None),
        _ => Err(format!("unknown pad name '{name}'")),
    }
}

/// The inputs a key name stands for: two for Shift, Ctrl and Alt (left
/// and right). A letter, a digit, a name below, or a virtual key in hex
/// (`0xBA`).
fn inputs_named(name: &str) -> Option<Vec<Input>> {
    let one = |i: Input| Some(vec![i]);
    if let [c] = name.as_bytes()
        && c.is_ascii_alphanumeric()
    {
        return one(Input::from(c.to_ascii_uppercase()));
    }
    let lower = name.to_ascii_lowercase();
    if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<u16>().ok())
        && (1..=12).contains(&n)
    {
        return one(0x6F + n);
    }
    if let Some(n) = lower
        .strip_prefix("num")
        .and_then(|n| n.parse::<u16>().ok())
        && n <= 9
    {
        return one(0x60 + n);
    }
    if let Some(hex) = lower.strip_prefix("0x") {
        return u8::from_str_radix(hex, 16)
            .ok()
            .map(|vk| vec![Input::from(vk)]);
    }
    match lower.as_str() {
        "space" => one(0x20),
        "enter" | "return" => one(0x0D),
        "esc" | "escape" => one(0x1B),
        "tab" => one(0x09),
        "backspace" | "back" => one(0x08),
        "up" => one(0x26),
        "down" => one(0x28),
        "left" => one(0x25),
        "right" => one(0x27),
        "shift" => Some(vec![0xA0, 0xA1]),
        "lshift" => one(0xA0),
        "rshift" => one(0xA1),
        "ctrl" | "control" => Some(vec![0xA2, 0xA3]),
        "lctrl" => one(0xA2),
        "rctrl" => one(0xA3),
        "alt" => Some(vec![0xA4, 0xA5]),
        "lalt" => one(0xA4),
        "ralt" => one(0xA5),
        "capslock" | "caps" => one(0x14),
        "insert" => one(0x2D),
        "delete" => one(0x2E),
        "home" => one(0x24),
        "end" => one(0x23),
        "pageup" => one(0x21),
        "pagedown" => one(0x22),
        "lmb" | "mouseleft" => one(MOUSE_BUTTON),
        "rmb" | "mouseright" => one(MOUSE_BUTTON + 1),
        "mmb" | "mousemiddle" => one(MOUSE_BUTTON + 2),
        "mouse4" => one(MOUSE_BUTTON + 3),
        "mouse5" => one(MOUSE_BUTTON + 4),
        "wheelup" => one(WHEEL_UP),
        "wheeldown" => one(WHEEL_DOWN),
        _ => None,
    }
}

/// A wheel notch as a short press of its input: the title reads the pad
/// about once a frame (~100 ms here), so a notch is held [`Wheel::hold`]
/// polls and let go [`Wheel::gap`] polls before the next one. At most
/// three notches wait; turning the other way drops those waiting.
/// `VICTORY_WHEEL_PULSE_MS=<ms>`: how long a notch is held (150; let go
/// two thirds of that).
#[derive(Clone, Copy, Debug)]
pub(super) struct Wheel {
    pending: i32,
    /// Polls left holding (> 0) or letting go (< 0) the current notch.
    phase: i32,
    input: Input,
    hold: i32,
    gap: i32,
}

impl Wheel {
    pub(super) fn new(hold: i32, gap: i32) -> Wheel {
        Wheel {
            pending: 0,
            phase: 0,
            input: WHEEL_UP,
            hold: hold.max(1),
            gap: gap.max(1),
        }
    }

    pub(super) fn from_env(poll: std::time::Duration) -> Wheel {
        static MS: LazyLock<u64> = LazyLock::new(|| {
            std::env::var("VICTORY_WHEEL_PULSE_MS")
                .ok()
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(150)
        });
        let polls = |ms: u64| (ms / poll.as_millis().max(1) as u64).max(1) as i32;
        Wheel::new(polls(*MS), polls(*MS * 2 / 3))
    }

    /// Takes `notches` (up positive) and returns the wheel input held now.
    pub(super) fn step(&mut self, notches: i32) -> Option<Input> {
        if notches != 0 {
            self.pending = if notches.signum() == self.pending.signum() {
                self.pending + notches
            } else {
                notches
            };
            self.pending = self.pending.clamp(-3, 3);
        }
        if self.phase > 0 {
            self.phase -= 1;
            if self.phase == 0 {
                self.phase = -self.gap;
            }
            return Some(self.input);
        }
        if self.phase < 0 {
            self.phase += 1;
            return None;
        }
        if self.pending == 0 {
            return None;
        }
        self.input = if self.pending > 0 {
            WHEEL_UP
        } else {
            WHEEL_DOWN
        };
        self.pending -= self.pending.signum();
        self.phase = self.hold - 1;
        if self.phase == 0 {
            self.phase = -self.gap;
        }
        Some(self.input)
    }
}

/// The keyboard and mouse's state between polls: the layout, the mode, the
/// toggle keys held at the last poll (a mode flips on a press, not while
/// held), the wheel's notches.
#[derive(Clone, Copy, Debug)]
pub(super) struct Controls {
    pub(super) map: &'static Keymap,
    pub(super) mode: Mode,
    toggle_held: bool,
    wheel: Wheel,
}

impl Controls {
    pub(super) fn new(map: &'static Keymap, wheel: Wheel) -> Controls {
        Controls {
            map,
            mode: Mode::Foot,
            toggle_held: false,
            wheel,
        }
    }

    /// The stand-in pad for the inputs `held` and `notches` of the wheel;
    /// flips the mode when a toggle key is pressed, and returns the mode
    /// then, if it changed. A key held across the flip (F) presses what it
    /// does in the new mode.
    pub(super) fn pad(&mut self, held: &[Input], notches: i32) -> (super::PadState, Option<Mode>) {
        let mut held = held.to_vec();
        held.extend(self.wheel.step(notches));
        let toggle = self.map.toggles(self.mode, &held);
        let pressed = toggle && !self.toggle_held;
        self.toggle_held = toggle;
        let changed = pressed.then(|| {
            self.mode = self.mode.flipped();
            self.mode
        });
        (self.map.pad(self.mode, &held), changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gameinput::{PadState, WALK_SPEED};

    fn pc() -> &'static Keymap {
        static PC_MAP: LazyLock<Keymap> = LazyLock::new(Keymap::pc);
        &PC_MAP
    }

    const LSHIFT: Input = 0xA0;
    const LCTRL: Input = 0xA2;
    const SPACE: Input = 0x20;
    const LMB: Input = MOUSE_BUTTON;
    const RMB: Input = MOUSE_BUTTON + 1;
    fn k(c: u8) -> Input {
        Input::from(c)
    }

    #[test]
    fn on_foot_the_pc_layout() {
        let map = pc();
        let pad = map.pad(Mode::Foot, &[k(b'W'), k(b'D'), LSHIFT, SPACE]);
        assert_eq!(
            pad,
            PadState {
                buttons: A | X,
                left_x: 1.0,
                left_y: 1.0,
                ..PadState::default()
            }
        );
        let buttons = |keys: &[Input]| map.pad(Mode::Foot, keys).buttons;
        assert_eq!(buttons(&[k(b'F')]), Y);
        assert_eq!(buttons(&[k(b'E')]), RIGHT_SHOULDER);
        assert_eq!(buttons(&[k(b'Q')]), RIGHT_SHOULDER);
        assert_eq!(buttons(&[k(b'R')]), B);
        assert_eq!(buttons(&[LCTRL]), LEFT_STICK);
        assert_eq!(buttons(&[0xA3]), LEFT_STICK, "right Ctrl too");
        assert_eq!(buttons(&[0x09]), LEFT_SHOULDER, "Tab: weapon wheel");
        assert_eq!(buttons(&[k(b'M')]), VIEW);
        assert_eq!(buttons(&[0x1B]), MENU);
        assert_eq!(buttons(&[k(b'H')]), UP);
        assert_eq!(buttons(&[0x0D, 0x08]), A | B, "menus: Enter and Backspace");
        assert_eq!(buttons(&[0x26, 0x28, 0x25, 0x27]), UP | DOWN | LEFT | RIGHT);
        assert_eq!(buttons(&[WHEEL_UP]), LEFT);
        assert_eq!(buttons(&[WHEEL_DOWN]), RIGHT);
        // Walking: the stick at walk speed, on a diagonal too.
        let pad = map.pad(Mode::Foot, &[k(b'X'), k(b'W'), k(b'D')]);
        assert!((pad.left_x.hypot(pad.left_y) - WALK_SPEED).abs() < 1e-6);
        assert_eq!(pad.buttons, 0);
        // I/J/K/L: the right stick, without a mouse.
        let pad = map.pad(Mode::Foot, &[k(b'L'), k(b'I')]);
        assert_eq!((pad.right_x, pad.right_y), (1.0, 1.0));
    }

    #[test]
    fn mouse_buttons_are_the_triggers_on_foot_and_lb_rb_driving() {
        let map = pc();
        let pad = map.pad(Mode::Foot, &[LMB]);
        assert_eq!(
            (pad.right_trigger, pad.left_trigger, pad.buttons),
            (1.0, 0.0, 0)
        );
        let pad = map.pad(Mode::Foot, &[RMB, LMB]);
        assert_eq!((pad.right_trigger, pad.left_trigger), (1.0, 1.0));
        assert_eq!(
            map.pad(Mode::Foot, &[MOUSE_BUTTON + 2]).buttons,
            RIGHT_STICK
        );
        // Driving: aim LB, fire RB, the triggers left to W and S.
        let pad = map.pad(Mode::Driving, &[RMB, LMB]);
        assert_eq!(
            (pad.buttons, pad.left_trigger, pad.right_trigger),
            (LEFT_SHOULDER | RIGHT_SHOULDER, 0.0, 0.0)
        );
    }

    #[test]
    fn driving_the_pc_layout() {
        let map = pc();
        let pad = map.pad(Mode::Driving, &[k(b'W'), k(b'A')]);
        assert_eq!(
            pad,
            PadState {
                right_trigger: 1.0,
                left_x: -1.0,
                ..PadState::default()
            }
        );
        let pad = map.pad(Mode::Driving, &[k(b'S'), k(b'D'), SPACE]);
        assert_eq!(
            pad,
            PadState {
                buttons: RIGHT_SHOULDER,
                left_trigger: 1.0,
                left_x: 1.0,
                ..PadState::default()
            }
        );
        let buttons = |keys: &[Input]| map.pad(Mode::Driving, keys).buttons;
        assert_eq!(buttons(&[k(b'F')]), Y, "F leaves the vehicle");
        assert_eq!(buttons(&[k(b'E')]), LEFT_STICK, "horn");
        assert_eq!(buttons(&[k(b'C')]), RIGHT_STICK, "look behind");
        assert_eq!(
            buttons(&[0x0D, 0x1B, k(b'M'), k(b'H')]),
            A | MENU | VIEW | UP,
            "[both] still applies"
        );
        // W never moves the stick in a vehicle.
        assert_eq!(map.pad(Mode::Driving, &[k(b'W')]).left_y, 0.0);
    }

    #[test]
    fn f_and_v_flip_the_mode_on_a_press() {
        let mut c = Controls::new(pc(), Wheel::new(2, 1));
        assert_eq!(c.mode, Mode::Foot);
        // F: Y and the driving mode, once while held.
        let (pad, changed) = c.pad(&[k(b'F')], 0);
        assert_eq!((pad.buttons, changed), (Y, Some(Mode::Driving)));
        let (pad, changed) = c.pad(&[k(b'F'), k(b'W')], 0);
        assert_eq!((pad.buttons, pad.right_trigger, changed), (Y, 1.0, None));
        assert_eq!(c.pad(&[], 0).1, None);
        // V flips alone, and presses nothing.
        let (pad, changed) = c.pad(&[k(b'V')], 0);
        assert_eq!((pad, changed), (PadState::default(), Some(Mode::Foot)));
        assert_eq!(c.pad(&[k(b'V')], 0).1, None);
        c.pad(&[], 0);
        // F and V together: one flip.
        assert_eq!(c.pad(&[k(b'F'), k(b'V')], 0).1, Some(Mode::Driving));
        assert_eq!(c.pad(&[k(b'F')], 0).1, None);
        assert_eq!(c.mode, Mode::Driving);
        assert_eq!(Mode::Driving.note(), "[driving]");
        assert_eq!(Mode::Foot.note(), "[on foot]");
    }

    #[test]
    fn wheel_notches_are_short_presses() {
        let mut w = Wheel::new(3, 2);
        // One notch up: held 3 polls, let go 2.
        let seen: Vec<_> = (0..7).map(|i| w.step(if i == 0 { 1 } else { 0 })).collect();
        assert_eq!(
            seen,
            [
                Some(WHEEL_UP),
                Some(WHEEL_UP),
                Some(WHEEL_UP),
                None,
                None,
                None,
                None
            ]
        );
        // Two notches down: two presses with a gap.
        let seen: Vec<_> = (0..10)
            .map(|i| w.step(if i == 0 { -2 } else { 0 }))
            .collect();
        let d = Some(WHEEL_DOWN);
        assert_eq!(seen, [d, d, d, None, None, d, d, d, None, None]);
        // Many notches: three at most wait; the other way drops them.
        w.step(10);
        assert_eq!(w.pending, 2);
        w.step(-1);
        assert_eq!(w.pending, -1);
    }

    #[test]
    fn a_file_rebinds_over_the_pc_layout() {
        let mut map = Keymap::pc();
        let errors = map.apply(
            "# mine\n[foot]\nW, Up = left_up # also the arrow\nQ = none\nG = lb + x\n[driving]\nSpace = a\n[nowhere]\nZ = jump\nP = \n",
        );
        assert_eq!(
            errors,
            [
                "line 8: unknown section [nowhere] (both, foot, driving)",
                "line 9: unknown pad name 'jump'",
                "line 10: unknown pad name ''"
            ]
        );
        assert_eq!(
            map.pad(Mode::Foot, &[0x26]).left_y,
            1.0,
            "Up moves on foot now"
        );
        assert_eq!(
            map.pad(Mode::Driving, &[0x26]).buttons,
            UP,
            "and is still the d-pad driving"
        );
        assert_eq!(map.buttons(Mode::Foot, k(b'Q')), 0);
        assert_eq!(map.buttons(Mode::Foot, k(b'G')), LEFT_SHOULDER | X);
        assert_eq!(map.buttons(Mode::Driving, SPACE), A);
        // `clear` empties a section: [both] shows through.
        let map = Keymap::parse("[both]\nW = up\n[foot]\nW = left_up\nclear\n").unwrap();
        assert_eq!(map.buttons(Mode::Foot, k(b'W')), UP);
        assert!(
            Keymap::parse("[both]\nNotAKey = a").unwrap_err()[0].contains("unknown key 'NotAKey'")
        );
        let map = Keymap::parse("F5 = view\nNum8 = up\n0xBA = b\nShift = a").unwrap();
        assert_eq!(map.buttons(Mode::Foot, 0x74), VIEW);
        assert_eq!(map.buttons(Mode::Foot, 0x68), UP);
        assert_eq!(map.buttons(Mode::Foot, 0xBA), B);
        assert_eq!(map.buttons(Mode::Driving, 0xA1), A, "right Shift too");
    }

    #[test]
    fn the_classic_layout_is_the_one_of_before() {
        let map = Keymap::parse(CLASSIC).unwrap();
        for mode in [Mode::Foot, Mode::Driving] {
            let pad = map.pad(
                mode,
                &[
                    k(b'W'),
                    k(b'Z'),
                    k(b'C'),
                    0x09,
                    k(b'Q'),
                    k(b'E'),
                    k(b'F'),
                    k(b'H'),
                    k(b'Y'),
                ],
            );
            assert_eq!(
                pad,
                PadState {
                    buttons: VIEW | LEFT_SHOULDER | RIGHT_SHOULDER | LEFT_STICK | RIGHT_STICK | Y,
                    left_trigger: 1.0,
                    right_trigger: 1.0,
                    left_y: 1.0,
                    ..PadState::default()
                }
            );
            assert_eq!(
                map.pad(mode, &[LMB, RMB]),
                PadState::default(),
                "no mouse buttons"
            );
            assert!(!map.toggles(mode, &[k(b'F'), k(b'V')]), "no modes");
        }
    }
}
