//! Remappable keyboard shortcuts.
//!
//! A shortcut is stored as text ("Ctrl+Shift+Z", "F9", "Delete") so the
//! preference file stays readable. Parsing and formatting are exact inverses,
//! and every command is resolved through the same table, so the interface, the
//! help window and the conflict check cannot drift apart.

use std::collections::BTreeMap;

use egui::{Key, Modifiers};
use serde::{Deserialize, Serialize};

/// A command that can be triggered from the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Command {
    New,
    Open,
    Save,
    SaveAs,
    Undo,
    Redo,
    SelectAll,
    Cut,
    Copy,
    Paste,
    Duplicate,
    Delete,
    Group,
    Ungroup,
    Escape,
    Fit,
    Grid,
    ZoomIn,
    ZoomOut,
    NudgeLeft,
    NudgeRight,
    NudgeUp,
    NudgeDown,
    ToolSelect,
    ToolLine,
    ToolRectangle,
    ToolEllipse,
    ToolStar,
    ToolText,
    PolygonTool,
    PencilTool,
    Preview,
    ToggleLayers,
    ToggleProperties,
    Enter,
    CommandPalette,
}

impl Command {
    /// Every remappable command, in the order shown in the preferences.
    pub const ALL: [Command; 36] = [
        Command::New,
        Command::Open,
        Command::Save,
        Command::SaveAs,
        Command::Undo,
        Command::Redo,
        Command::SelectAll,
        Command::Cut,
        Command::Copy,
        Command::Paste,
        Command::Duplicate,
        Command::Delete,
        Command::Group,
        Command::Ungroup,
        Command::Escape,
        Command::Fit,
        Command::Grid,
        Command::ZoomIn,
        Command::ZoomOut,
        Command::NudgeLeft,
        Command::NudgeRight,
        Command::NudgeUp,
        Command::NudgeDown,
        Command::ToolSelect,
        Command::ToolLine,
        Command::ToolRectangle,
        Command::ToolEllipse,
        Command::ToolStar,
        Command::ToolText,
        Command::PolygonTool,
        Command::PencilTool,
        Command::Preview,
        Command::ToggleLayers,
        Command::ToggleProperties,
        Command::Enter,
        Command::CommandPalette,
    ];

    /// Stable identifier used in the preference file.
    pub fn id(self) -> &'static str {
        match self {
            Command::New => "new",
            Command::Open => "open",
            Command::Save => "save",
            Command::SaveAs => "save_as",
            Command::Undo => "undo",
            Command::Redo => "redo",
            Command::SelectAll => "select_all",
            Command::Cut => "cut",
            Command::Copy => "copy",
            Command::Paste => "paste",
            Command::Duplicate => "duplicate",
            Command::Delete => "delete",
            Command::Group => "group",
            Command::Ungroup => "ungroup",
            Command::Escape => "escape",
            Command::Fit => "fit",
            Command::Grid => "grid",
            Command::ZoomIn => "zoom_in",
            Command::ZoomOut => "zoom_out",
            Command::NudgeLeft => "nudge_left",
            Command::NudgeRight => "nudge_right",
            Command::NudgeUp => "nudge_up",
            Command::NudgeDown => "nudge_down",
            Command::ToolSelect => "tool_select",
            Command::ToolLine => "tool_line",
            Command::ToolRectangle => "tool_rectangle",
            Command::ToolEllipse => "tool_ellipse",
            Command::ToolStar => "tool_star",
            Command::ToolText => "tool_text",
            Command::PolygonTool => "tool_polygon",
            Command::PencilTool => "tool_pencil",
            Command::Preview => "preview",
            Command::ToggleLayers => "toggle_layers",
            Command::ToggleProperties => "toggle_properties",
            Command::Enter => "enter",
            Command::CommandPalette => "command_palette",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Command::New => "New project",
            Command::Open => "Open project",
            Command::Save => "Save",
            Command::SaveAs => "Save as",
            Command::Undo => "Undo",
            Command::Redo => "Redo",
            Command::SelectAll => "Select all",
            Command::Cut => "Cut",
            Command::Copy => "Copy",
            Command::Paste => "Paste",
            Command::Duplicate => "Duplicate",
            Command::Delete => "Delete selection",
            Command::Group => "Group",
            Command::Ungroup => "Ungroup",
            Command::Escape => "Cancel / deselect",
            Command::Fit => "Fit page",
            Command::Grid => "Toggle grid",
            Command::ZoomIn => "Zoom in",
            Command::ZoomOut => "Zoom out",
            Command::NudgeLeft => "Nudge left",
            Command::NudgeRight => "Nudge right",
            Command::NudgeUp => "Nudge up",
            Command::NudgeDown => "Nudge down",
            Command::ToolSelect => "Select tool",
            Command::ToolLine => "Line tool",
            Command::ToolRectangle => "Rectangle tool",
            Command::ToolEllipse => "Ellipse tool",
            Command::ToolStar => "Star tool",
            Command::ToolText => "Text tool",
            Command::PolygonTool => "Polygon tool",
            Command::PencilTool => "Freehand tool",
            Command::Preview => "Preview and G-code",
            Command::ToggleLayers => "Layers panel",
            Command::ToggleProperties => "Properties panel",
            Command::Enter => "Enter / finish",
            Command::CommandPalette => "Command palette",
        }
    }

    /// The binding used when the preference file has none.
    pub fn default_chord(self) -> &'static str {
        match self {
            Command::New => "Ctrl+N",
            Command::Open => "Ctrl+O",
            Command::Save => "Ctrl+S",
            Command::SaveAs => "Ctrl+Shift+S",
            Command::Undo => "Ctrl+Z",
            Command::Redo => "Ctrl+Y",
            Command::SelectAll => "Ctrl+A",
            Command::Cut => "Ctrl+X",
            Command::Copy => "Ctrl+C",
            Command::Paste => "Ctrl+V",
            Command::Duplicate => "Ctrl+D",
            Command::Delete => "Delete",
            Command::Group => "Ctrl+G",
            Command::Ungroup => "Ctrl+Shift+G",
            Command::Escape => "Escape",
            Command::Fit => "F",
            Command::Grid => "G",
            Command::ZoomIn => "Plus",
            Command::ZoomOut => "Minus",
            Command::NudgeLeft => "ArrowLeft",
            Command::NudgeRight => "ArrowRight",
            Command::NudgeUp => "ArrowUp",
            Command::NudgeDown => "ArrowDown",
            Command::ToolSelect => "V",
            Command::ToolLine => "L",
            Command::ToolRectangle => "R",
            Command::ToolEllipse => "E",
            Command::ToolStar => "S",
            Command::ToolText => "T",
            Command::PolygonTool => "P",
            Command::PencilTool => "B",
            Command::Preview => "Ctrl+P",
            Command::ToggleLayers => "F9",
            Command::ToggleProperties => "F10",
            Command::Enter => "Enter",
            Command::CommandPalette => "Ctrl+K",
        }
    }
}

/// The current binding of every command, with defaults for missing entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keymap {
    #[serde(default)]
    bindings: BTreeMap<String, String>,
}

impl Default for Keymap {
    fn default() -> Self {
        let mut bindings = BTreeMap::new();
        for command in Command::ALL {
            bindings.insert(
                command.id().to_string(),
                command.default_chord().to_string(),
            );
        }
        Self { bindings }
    }
}

impl Keymap {
    /// The chord text for a command, falling back to its default.
    pub fn chord(&self, command: Command) -> &str {
        self.bindings
            .get(command.id())
            .map_or(command.default_chord(), String::as_str)
    }

    /// Bind a key combination, rejecting a chord another command already uses.
    pub fn assign(
        &mut self,
        command: Command,
        key: Key,
        modifiers: Modifiers,
    ) -> Result<(), String> {
        let chord = format_chord(key, modifiers)?;
        if let Some(other) = self.conflict(command, key, modifiers) {
            return Err(format!("{chord} is already used by \"{}\"", other.label()));
        }
        self.bindings.insert(command.id().to_string(), chord);
        Ok(())
    }

    /// Bind from text, used by tests and future import.
    pub fn assign_text(&mut self, command: Command, chord: &str) -> Result<(), String> {
        let (modifiers, key) = parse_chord(chord)?;
        self.assign(command, key, modifiers)
    }

    /// The command already using this combination, if any.
    pub fn conflict(&self, command: Command, key: Key, modifiers: Modifiers) -> Option<Command> {
        Command::ALL.into_iter().find(|other| {
            if *other == command {
                return false;
            }
            parse_chord(self.chord(*other)).is_ok_and(|(other_modifiers, other_key)| {
                other_key == key && same_modifiers(other_modifiers, modifiers)
            })
        })
    }

    pub fn reset(&mut self, command: Command) {
        self.bindings.remove(command.id());
    }

    pub fn reset_all(&mut self) {
        *self = Self::default();
    }

    /// Every binding must parse and no two commands may share one.
    pub fn validate(&self) -> Result<(), String> {
        let mut seen: Vec<(Modifiers, Key, Command)> = Vec::new();
        for command in Command::ALL {
            let (modifiers, key) = parse_chord(self.chord(command))
                .map_err(|error| format!("{}: {error}", command.label()))?;
            if let Some((_, _, other)) = seen.iter().find(|(other_modifiers, other_key, _)| {
                *other_key == key && same_modifiers(*other_modifiers, modifiers)
            }) {
                return Err(format!(
                    "{} and {} share {}",
                    other.label(),
                    command.label(),
                    self.chord(command)
                ));
            }
            seen.push((modifiers, key, command));
        }
        Ok(())
    }
}

fn same_modifiers(left: Modifiers, right: Modifiers) -> bool {
    left.ctrl == right.ctrl && left.shift == right.shift && left.alt == right.alt
}

/// Parse "Ctrl+Shift+Z" into modifiers and a key.
pub fn parse_chord(text: &str) -> Result<(Modifiers, Key), String> {
    let mut modifiers = Modifiers::NONE;
    let mut key = None;
    for part in text.split('+') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CTRL,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" | "option" => modifiers |= Modifiers::ALT,
            _ => {
                if key.is_some() {
                    return Err(format!("'{text}' names more than one key"));
                }
                key = Some(key_from_name(part).ok_or_else(|| format!("unknown key '{part}'"))?);
            }
        }
    }
    let key = key.ok_or_else(|| format!("'{text}' has no key"))?;
    Ok((modifiers, key))
}

/// Format a key combination the way it is stored and shown.
pub fn format_chord(key: Key, modifiers: Modifiers) -> Result<String, String> {
    let name = key_name(key).ok_or_else(|| "that key cannot be used".to_string())?;
    let mut chord = String::new();
    if modifiers.ctrl {
        chord.push_str("Ctrl+");
    }
    if modifiers.shift {
        chord.push_str("Shift+");
    }
    if modifiers.alt {
        chord.push_str("Alt+");
    }
    chord.push_str(name);
    Ok(chord)
}

/// The stored name of a key, or `None` for keys that cannot be bound.
pub fn key_name(key: Key) -> Option<&'static str> {
    Some(match key {
        Key::A => "A",
        Key::B => "B",
        Key::C => "C",
        Key::D => "D",
        Key::E => "E",
        Key::F => "F",
        Key::G => "G",
        Key::H => "H",
        Key::I => "I",
        Key::J => "J",
        Key::K => "K",
        Key::L => "L",
        Key::M => "M",
        Key::N => "N",
        Key::O => "O",
        Key::P => "P",
        Key::Q => "Q",
        Key::R => "R",
        Key::S => "S",
        Key::T => "T",
        Key::U => "U",
        Key::V => "V",
        Key::W => "W",
        Key::X => "X",
        Key::Y => "Y",
        Key::Z => "Z",
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num7 => "7",
        Key::Num8 => "8",
        Key::Num9 => "9",
        Key::F1 => "F1",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",
        Key::ArrowLeft => "ArrowLeft",
        Key::ArrowRight => "ArrowRight",
        Key::ArrowUp => "ArrowUp",
        Key::ArrowDown => "ArrowDown",
        Key::Escape => "Escape",
        Key::Tab => "Tab",
        Key::Backspace => "Backspace",
        Key::Enter => "Enter",
        Key::Space => "Space",
        Key::Insert => "Insert",
        Key::Delete => "Delete",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::Plus => "Plus",
        Key::Minus => "Minus",
        Key::Equals => "Equals",
        Key::Comma => "Comma",
        Key::Period => "Period",
        Key::Slash => "Slash",
        Key::Semicolon => "Semicolon",
        Key::Quote => "Quote",
        Key::Backtick => "Backtick",
        Key::OpenBracket => "OpenBracket",
        Key::CloseBracket => "CloseBracket",
        Key::Backslash => "Backslash",
        _ => return None,
    })
}

/// The key for a stored name, case-insensitive, with a few readable aliases.
pub fn key_from_name(name: &str) -> Option<Key> {
    let trimmed = name.trim();
    if trimmed.chars().count() == 1 {
        let character = trimmed.chars().next()?.to_ascii_uppercase();
        return match character {
            'A' => Some(Key::A),
            'B' => Some(Key::B),
            'C' => Some(Key::C),
            'D' => Some(Key::D),
            'E' => Some(Key::E),
            'F' => Some(Key::F),
            'G' => Some(Key::G),
            'H' => Some(Key::H),
            'I' => Some(Key::I),
            'J' => Some(Key::J),
            'K' => Some(Key::K),
            'L' => Some(Key::L),
            'M' => Some(Key::M),
            'N' => Some(Key::N),
            'O' => Some(Key::O),
            'P' => Some(Key::P),
            'Q' => Some(Key::Q),
            'R' => Some(Key::R),
            'S' => Some(Key::S),
            'T' => Some(Key::T),
            'U' => Some(Key::U),
            'V' => Some(Key::V),
            'W' => Some(Key::W),
            'X' => Some(Key::X),
            'Y' => Some(Key::Y),
            'Z' => Some(Key::Z),
            '0' => Some(Key::Num0),
            '1' => Some(Key::Num1),
            '2' => Some(Key::Num2),
            '3' => Some(Key::Num3),
            '4' => Some(Key::Num4),
            '5' => Some(Key::Num5),
            '6' => Some(Key::Num6),
            '7' => Some(Key::Num7),
            '8' => Some(Key::Num8),
            '9' => Some(Key::Num9),
            _ => None,
        };
    }
    Some(match trimmed.to_ascii_lowercase().as_str() {
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        "left" | "arrowleft" => Key::ArrowLeft,
        "right" | "arrowright" => Key::ArrowRight,
        "up" | "arrowup" => Key::ArrowUp,
        "down" | "arrowdown" => Key::ArrowDown,
        "esc" | "escape" => Key::Escape,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "enter" | "return" => Key::Enter,
        "space" => Key::Space,
        "insert" => Key::Insert,
        "delete" | "del" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "plus" => Key::Plus,
        "minus" => Key::Minus,
        "equals" => Key::Equals,
        "comma" => Key::Comma,
        "period" => Key::Period,
        "slash" => Key::Slash,
        "semicolon" => Key::Semicolon,
        "quote" => Key::Quote,
        "backtick" => Key::Backtick,
        "openbracket" => Key::OpenBracket,
        "closebracket" => Key::CloseBracket,
        "backslash" => Key::Backslash,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{Command, Keymap, format_chord, key_from_name, key_name, parse_chord};
    use egui::Key;

    #[test]
    fn default_bindings_are_unique_and_parse() {
        let keymap = Keymap::default();
        assert!(keymap.validate().is_ok());
        for command in Command::ALL {
            let chord = keymap.chord(command);
            let (modifiers, key) = parse_chord(chord).unwrap();
            assert_eq!(format_chord(key, modifiers).unwrap(), chord);
        }
    }

    #[test]
    fn every_command_has_a_distinct_id_and_label() {
        let mut ids = Vec::new();
        let mut labels = Vec::new();
        for command in Command::ALL {
            assert_ne!(command.id(), "");
            assert_ne!(command.label(), "");
            assert!(!ids.contains(&command.id()));
            assert!(!labels.contains(&command.label()));
            ids.push(command.id());
            labels.push(command.label());
        }
    }

    #[test]
    fn chords_round_trip_through_text_and_keys() {
        for key in [
            Key::A,
            Key::Num7,
            Key::F9,
            Key::ArrowLeft,
            Key::Delete,
            Key::Plus,
            Key::OpenBracket,
        ] {
            let name = key_name(key).unwrap();
            assert_eq!(key_from_name(name), Some(key));
            assert_eq!(key_from_name(&name.to_lowercase()), Some(key));
        }
        let (modifiers, key) = parse_chord("ctrl + shift + z").unwrap();
        assert!(modifiers.ctrl && modifiers.shift && !modifiers.alt);
        assert_eq!(key, Key::Z);
        assert_eq!(format_chord(key, modifiers).unwrap(), "Ctrl+Shift+Z");
    }

    #[test]
    fn bad_chords_are_rejected() {
        assert!(parse_chord("").is_err());
        assert!(parse_chord("Ctrl+").is_err());
        assert!(parse_chord("Ctrl+Z+X").is_err());
        assert!(parse_chord("Ctrl+NoSuchKey").is_err());
        let mut keymap = Keymap::default();
        assert!(keymap.assign_text(Command::New, "Ctrl+NoSuchKey").is_err());
    }

    #[test]
    fn assigning_a_used_chord_is_reported() {
        let mut keymap = Keymap::default();
        let error = keymap.assign_text(Command::New, "Ctrl+S").unwrap_err();
        assert!(error.contains("Save"), "{error}");
        // Assigning a free combination works and is reflected in the chord.
        keymap.assign_text(Command::New, "Ctrl+Shift+N").unwrap();
        assert_eq!(keymap.chord(Command::New), "Ctrl+Shift+N");
        assert!(keymap.validate().is_ok());
    }

    #[test]
    fn reset_restores_the_default_binding() {
        let mut keymap = Keymap::default();
        keymap.assign_text(Command::New, "Ctrl+Shift+N").unwrap();
        keymap.reset(Command::New);
        assert_eq!(keymap.chord(Command::New), Command::New.default_chord());
        keymap.reset_all();
        assert_eq!(keymap, Keymap::default());
    }

    #[test]
    fn a_hand_edited_duplicate_is_rejected_by_validate() {
        let mut keymap = Keymap::default();
        keymap
            .bindings
            .insert("new".to_string(), "Ctrl+S".to_string());
        assert!(keymap.validate().is_err());
        keymap.bindings.clear();
        // An empty map means every command uses its default.
        assert!(keymap.validate().is_ok());
    }

    #[test]
    fn the_keymap_round_trips_through_json() {
        let mut keymap = Keymap::default();
        keymap.assign_text(Command::Grid, "Shift+G").unwrap();
        let json = serde_json::to_string(&keymap).unwrap();
        let loaded: Keymap = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, keymap);
        assert_eq!(loaded.chord(Command::Grid), "Shift+G");
        // A missing or partial map still resolves every command.
        let partial: Keymap = serde_json::from_str(r#"{"bindings":{"grid":"G"}}"#).unwrap();
        assert_eq!(partial.chord(Command::Grid), "G");
        assert_eq!(partial.chord(Command::Save), "Ctrl+S");
    }
}
