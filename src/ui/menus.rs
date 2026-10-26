use crate::ui::icons::Icon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    New,
    Open,
    Save,
    Undo,
    Redo,
    Quit,
    ZoomIn,
    ZoomOut,
    Grid,
    Import,
    Export,
}

impl MenuAction {
    pub fn label(self) -> &'static str {
        match self {
            MenuAction::New => "New",
            MenuAction::Open => "Open",
            MenuAction::Save => "Save",
            MenuAction::Undo => "Undo",
            MenuAction::Redo => "Redo",
            MenuAction::Quit => "Quit",
            MenuAction::ZoomIn => "Zoom in",
            MenuAction::ZoomOut => "Zoom out",
            MenuAction::Grid => "Grid",
            MenuAction::Import => "Import",
            MenuAction::Export => "Export",
        }
    }
    pub fn icon(self) -> Icon {
        match self {
            MenuAction::New => Icon::File,
            MenuAction::Open | MenuAction::Import => Icon::FolderOpen,
            MenuAction::Save | MenuAction::Export => Icon::Save,
            MenuAction::Undo => Icon::Undo,
            MenuAction::Redo => Icon::Redo,
            MenuAction::Quit => Icon::Delete,
            MenuAction::ZoomIn => Icon::ZoomIn,
            MenuAction::ZoomOut => Icon::ZoomOut,
            MenuAction::Grid => Icon::Grid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuOrientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub action: MenuAction,
    pub enabled: bool,
}

impl MenuItem {
    pub fn new(action: MenuAction) -> Self {
        Self {
            action,
            enabled: true,
        }
    }
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

#[derive(Debug, Clone)]
pub struct Menu {
    pub title: &'static str,
    pub icon: Icon,
    pub items: Vec<MenuItem>,
    pub orientation: MenuOrientation,
}

impl Menu {
    pub fn new(title: &'static str, icon: Icon) -> Self {
        Self {
            title,
            icon,
            items: Vec::new(),
            orientation: MenuOrientation::Vertical,
        }
    }
    pub fn with_items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        self
    }
    pub fn horizontal(mut self) -> Self {
        self.orientation = MenuOrientation::Horizontal;
        self
    }
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct MenuBar {
    pub menus: Vec<Menu>,
}

impl MenuBar {
    pub fn new() -> Self {
        Self { menus: Vec::new() }
    }
    pub fn add(mut self, menu: Menu) -> Self {
        self.menus.push(menu);
        self
    }
    pub fn menu_count(&self) -> usize {
        self.menus.len()
    }
    pub fn total_items(&self) -> usize {
        self.menus.iter().map(Menu::item_count).sum()
    }
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the default menu bar for `BeamWright`.
pub fn default_menu_bar() -> MenuBar {
    MenuBar::new()
        .add(Menu::new("File", Icon::File).with_items(vec![
            MenuItem::new(MenuAction::New),
            MenuItem::new(MenuAction::Open),
            MenuItem::new(MenuAction::Save),
            MenuItem::new(MenuAction::Import),
            MenuItem::new(MenuAction::Export),
            MenuItem::new(MenuAction::Quit),
        ]))
        .add(Menu::new("Edit", Icon::Undo).with_items(vec![
            MenuItem::new(MenuAction::Undo),
            MenuItem::new(MenuAction::Redo),
        ]))
        .add(Menu::new("View", Icon::View).with_items(vec![
            MenuItem::new(MenuAction::ZoomIn),
            MenuItem::new(MenuAction::ZoomOut),
            MenuItem::new(MenuAction::Grid),
        ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_action_labels() {
        assert_eq!(MenuAction::New.label(), "New");
        assert_eq!(MenuAction::Undo.label(), "Undo");
        assert_eq!(MenuAction::ZoomIn.label(), "Zoom in");
    }

    #[test]
    fn menu_action_icons() {
        assert_eq!(MenuAction::New.icon(), Icon::File);
        assert_eq!(MenuAction::Save.icon(), Icon::Save);
    }

    #[test]
    fn menu_item_enabled() {
        let it = MenuItem::new(MenuAction::Save).disabled();
        assert!(!it.enabled);
    }

    #[test]
    fn menu_empty() {
        let m = Menu::new("X", Icon::File);
        assert!(m.is_empty());
        assert_eq!(m.item_count(), 0);
    }

    #[test]
    fn menu_with_items() {
        let m = Menu::new("File", Icon::File).with_items(vec![
            MenuItem::new(MenuAction::New),
            MenuItem::new(MenuAction::Open),
        ]);
        assert_eq!(m.item_count(), 2);
        assert!(!m.is_empty());
    }

    #[test]
    fn menu_bar_default() {
        let bar = default_menu_bar();
        assert_eq!(bar.menu_count(), 3);
        assert!(bar.total_items() > 0);
    }

    #[test]
    fn menu_orientation() {
        let m = Menu::new("X", Icon::File).horizontal();
        assert_eq!(m.orientation, MenuOrientation::Horizontal);
    }

    #[test]
    fn menu_action_equality() {
        assert_eq!(MenuAction::New, MenuAction::New);
        assert_ne!(MenuAction::New, MenuAction::Open);
    }
}
