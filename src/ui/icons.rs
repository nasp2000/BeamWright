/// Simple icon glyphs used in menus and toolbars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    File,
    FolderOpen,
    Save,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
    Draw,
    Arrange,
    Operations,
    Materials,
    View,
    ZoomIn,
    ZoomOut,
    Grid,
    Select,
    Line,
    Rect,
    Ellipse,
    Pan,
}

impl Icon {
    pub fn glyph(self) -> &'static str {
        match self {
            Icon::File => "📄",
            Icon::FolderOpen => "📂",
            Icon::Save => "💾",
            Icon::Undo => "↩",
            Icon::Redo => "↪",
            Icon::Cut => "✂",
            Icon::Copy => "📋",
            Icon::Paste => "📌",
            Icon::Delete => "🗑",
            Icon::Draw => "✏",
            Icon::Arrange => "📐",
            Icon::Operations => "⚙",
            Icon::Materials => "🧱",
            Icon::View => "👁",
            Icon::ZoomIn => "🔍+",
            Icon::ZoomOut => "🔍-",
            Icon::Grid => "▦",
            Icon::Select => "↖",
            Icon::Line => "╱",
            Icon::Rect => "▭",
            Icon::Ellipse => "⬭",
            Icon::Pan => "✋",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Icon::File => "File",
            Icon::FolderOpen => "Open",
            Icon::Save => "Save",
            Icon::Undo => "Undo",
            Icon::Redo => "Redo",
            Icon::Cut => "Cut",
            Icon::Copy => "Copy",
            Icon::Paste => "Paste",
            Icon::Delete => "Delete",
            Icon::Draw => "Draw",
            Icon::Arrange => "Arrange",
            Icon::Operations => "Operations",
            Icon::Materials => "Materials",
            Icon::View => "View",
            Icon::ZoomIn => "Zoom in",
            Icon::ZoomOut => "Zoom out",
            Icon::Grid => "Grid",
            Icon::Select => "Select",
            Icon::Line => "Line",
            Icon::Rect => "Rectangle",
            Icon::Ellipse => "Ellipse",
            Icon::Pan => "Pan",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Icon;

    #[test]
    fn glyphs_non_empty() {
        for i in [
            Icon::File,
            Icon::Save,
            Icon::Undo,
            Icon::Redo,
            Icon::Select,
            Icon::Line,
            Icon::Rect,
            Icon::Ellipse,
            Icon::Pan,
            Icon::Grid,
        ] {
            assert_ne!(i.glyph(), "");
        }
    }

    #[test]
    fn labels_non_empty() {
        for i in [Icon::File, Icon::Save, Icon::Undo, Icon::Redo] {
            assert_ne!(i.label(), "");
        }
    }

    #[test]
    fn equality() {
        assert_eq!(Icon::File, Icon::File);
        assert_ne!(Icon::File, Icon::Save);
    }
}
