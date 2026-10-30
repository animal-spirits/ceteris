//! Command registry (ADR 0188): every action is a named command, and keys, buttons and menus
//! reach it through the registry. The shell carries the Time and Map groups of the standard
//! keymap; the palette, vim mode and the rest arrive with P3, whose registry this becomes.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    TogglePause,
    Run,
    Pause,
    Speed(u8),
    Faster,
    Slower,
    Step,
    /// Pan by a fraction of the viewport, right and down.
    Pan(i8, i8),
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

impl Command {
    pub fn name(self) -> String {
        match self {
            Self::TogglePause => "time.toggle-pause".into(),
            Self::Run => "time.run".into(),
            Self::Pause => "time.pause".into(),
            Self::Speed(s) => format!("time.speed {s}"),
            Self::Faster => "time.faster".into(),
            Self::Slower => "time.slower".into(),
            Self::Step => "time.step".into(),
            Self::Pan(x, y) => format!("map.pan {x} {y}"),
            Self::ZoomIn => "map.zoom-in".into(),
            Self::ZoomOut => "map.zoom-out".into(),
            Self::ZoomReset => "map.zoom-reset".into(),
        }
    }
}

/// The standard keymap's Time and Map groups (0188), as chords in `chord`'s form.
pub const STANDARD: &[(&str, Command)] = &[
    ("Space", Command::TogglePause),
    ("1", Command::Speed(1)),
    ("2", Command::Speed(2)),
    ("3", Command::Speed(3)),
    ("4", Command::Speed(4)),
    ("5", Command::Speed(5)),
    ("+", Command::Faster),
    ("=", Command::Faster),
    ("-", Command::Slower),
    ("F5", Command::Run),
    ("Shift+F5", Command::Pause),
    ("F10", Command::Step),
    ("W", Command::Pan(0, -1)),
    ("A", Command::Pan(-1, 0)),
    ("S", Command::Pan(0, 1)),
    ("D", Command::Pan(1, 0)),
    ("Ctrl+=", Command::ZoomIn),
    ("Ctrl++", Command::ZoomIn),
    ("Ctrl+-", Command::ZoomOut),
    ("Ctrl+0", Command::ZoomReset),
];

/// A chord as the keymap names it: modifiers in Ctrl, Alt, Shift order, then the key, with
/// letters upper case. Shift is dropped for printable keys, whose text already carries it.
pub fn chord(key: &str, ctrl: bool, alt: bool, shift: bool) -> String {
    let printable = key.chars().count() == 1;
    let mut out = String::new();
    for (on, name) in [
        (ctrl, "Ctrl+"),
        (alt, "Alt+"),
        (shift && !printable, "Shift+"),
    ] {
        if on {
            out.push_str(name);
        }
    }
    if printable {
        out.push_str(&key.to_uppercase());
    } else {
        out.push_str(key);
    }
    out
}

/// A command by its registry name, for buttons and menus.
pub fn by_name(name: &str) -> Option<Command> {
    STANDARD.iter().map(|&(_, c)| c).find(|c| c.name() == name)
}

pub fn lookup(chord: &str) -> Option<Command> {
    STANDARD
        .iter()
        .find(|(c, _)| *c == chord)
        .map(|&(_, cmd)| cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_bindings_share_a_chord() {
        let mut chords: Vec<_> = STANDARD.iter().map(|(c, _)| *c).collect();
        chords.sort_unstable();
        let n = chords.len();
        chords.dedup();
        assert_eq!(chords.len(), n);
    }

    #[test]
    fn chords_normalise_case_and_modifier_order() {
        assert_eq!(chord("w", false, false, false), "W");
        assert_eq!(chord("W", false, false, true), "W");
        assert_eq!(chord("F5", false, false, true), "Shift+F5");
        assert_eq!(chord("Space", false, false, false), "Space");
        assert_eq!(chord("=", true, false, false), "Ctrl+=");
        assert_eq!(lookup(&chord(" ", false, false, false)), None);
        assert_eq!(lookup("Space"), Some(Command::TogglePause));
        assert_eq!(by_name("time.speed 4"), Some(Command::Speed(4)));
    }

    #[test]
    fn names_are_unique() {
        let mut names: Vec<_> = STANDARD.iter().map(|(_, c)| c.name()).collect();
        names.sort_unstable();
        names.dedup();
        // Faster and zoom-in each have two chords.
        assert_eq!(names.len(), STANDARD.len() - 2);
    }
}
