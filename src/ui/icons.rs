#![allow(clippy::too_many_lines)]

//! Icons for menus and panels.
//!
//! The icons are drawn in code on a 16×16 grid and rasterised once into a 32×32
//! texture per icon, so no extra dependency or image asset is needed. They are
//! white with an alpha channel and are tinted with the widget text colour by
//! `egui`, which keeps them legible in both themes and dimmed when disabled.
//!
//! Integration goes through [`button`], [`button_enabled`], [`selectable`],
//! [`selectable_value`] and [`checkbox`], which keep the standard egui widgets
//! (hover, keyboard activation, AccessKit) and only add the leading icon.

use egui::{
    Color32, ColorImage, Context, Image, Response, TextureHandle, TextureOptions, Ui, Vec2,
    WidgetText,
};

/// One icon. Variants are grouped by the menu that uses them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    // Top-level menus.
    File,
    Edit,
    Draw,
    Operations,
    Materials,
    Arrange,
    Align,
    Profiles,
    View,
    Help,
    // File.
    New,
    Open,
    Save,
    Import,
    Export,
    Page,
    Image,
    Recover,
    Exit,
    // Edit.
    Undo,
    Redo,
    SelectAll,
    Cut,
    Copy,
    Paste,
    Duplicate,
    Delete,
    Invert,
    Lock,
    Unlock,
    Preferences,
    Group,
    Ungroup,
    // Draw tools.
    Select,
    Line,
    Rectangle,
    Ellipse,
    Polygon,
    Star,
    Text,
    Measure,
    Bezier,
    Freehand,
    Pan,
    Nodes,
    Trim,
    Join,
    // Operations.
    Plus,
    Compare,
    Play,
    Clock,
    Check,
    Gear,
    Layers,
    Book,
    // Arrange.
    ToFront,
    ToBack,
    Center,
    DistributeH,
    DistributeV,
    RotateLeft,
    RotateRight,
    FlipH,
    FlipV,
    ScaleUp,
    ScaleDown,
    TextOutline,
    TextOnPath,
    TransformCenter,
    Skew,
    GridArray,
    ClosePath,
    Simplify,
    Repeat,
    Nest,
    FindDuplicates,
    Intersections,
    Boolean,
    BooleanUnion,
    BooleanIntersection,
    BooleanDifference,
    BooleanExclusion,
    OutlineStroke,
    Offset,
    // Align.
    AlignLeft,
    AlignCenterH,
    AlignRight,
    AlignTop,
    AlignCenterV,
    AlignBottom,
    // View.
    ZoomIn,
    ZoomOut,
    ZoomFit,
    ZoomSelection,
    Grid,
    Rulers,
    Guides,
    Snap,
    SnapCenter,
    SnapMidpoint,
    SnapLimits,
    Toolpaths,
    Panels,
    DarkMode,
    // Help and misc.
    Search,
    Info,
    Palette,
    Pencil,
    Eye,
}

impl Icon {
    /// Every icon, used by tests to keep the table honest.
    pub const ALL: &'static [Icon] = &[
        Icon::File,
        Icon::Edit,
        Icon::Draw,
        Icon::Operations,
        Icon::Materials,
        Icon::Arrange,
        Icon::Align,
        Icon::Profiles,
        Icon::View,
        Icon::Help,
        Icon::New,
        Icon::Open,
        Icon::Save,
        Icon::Import,
        Icon::Export,
        Icon::Page,
        Icon::Image,
        Icon::Recover,
        Icon::Exit,
        Icon::Undo,
        Icon::Redo,
        Icon::SelectAll,
        Icon::Cut,
        Icon::Copy,
        Icon::Paste,
        Icon::Duplicate,
        Icon::Delete,
        Icon::Invert,
        Icon::Lock,
        Icon::Unlock,
        Icon::Preferences,
        Icon::Group,
        Icon::Ungroup,
        Icon::Select,
        Icon::Line,
        Icon::Rectangle,
        Icon::Ellipse,
        Icon::Polygon,
        Icon::Star,
        Icon::Text,
        Icon::Measure,
        Icon::Bezier,
        Icon::Freehand,
        Icon::Pan,
        Icon::Nodes,
        Icon::Trim,
        Icon::Join,
        Icon::Plus,
        Icon::Compare,
        Icon::Play,
        Icon::Clock,
        Icon::Check,
        Icon::Gear,
        Icon::Layers,
        Icon::Book,
        Icon::ToFront,
        Icon::ToBack,
        Icon::Center,
        Icon::DistributeH,
        Icon::DistributeV,
        Icon::RotateLeft,
        Icon::RotateRight,
        Icon::FlipH,
        Icon::FlipV,
        Icon::ScaleUp,
        Icon::ScaleDown,
        Icon::TextOutline,
        Icon::TextOnPath,
        Icon::TransformCenter,
        Icon::Skew,
        Icon::GridArray,
        Icon::ClosePath,
        Icon::Simplify,
        Icon::Repeat,
        Icon::Nest,
        Icon::FindDuplicates,
        Icon::Intersections,
        Icon::Boolean,
        Icon::BooleanUnion,
        Icon::BooleanIntersection,
        Icon::BooleanDifference,
        Icon::BooleanExclusion,
        Icon::OutlineStroke,
        Icon::Offset,
        Icon::AlignLeft,
        Icon::AlignCenterH,
        Icon::AlignRight,
        Icon::AlignTop,
        Icon::AlignCenterV,
        Icon::AlignBottom,
        Icon::ZoomIn,
        Icon::ZoomOut,
        Icon::ZoomFit,
        Icon::ZoomSelection,
        Icon::Grid,
        Icon::Rulers,
        Icon::Guides,
        Icon::Snap,
        Icon::SnapCenter,
        Icon::SnapMidpoint,
        Icon::SnapLimits,
        Icon::Toolpaths,
        Icon::Panels,
        Icon::DarkMode,
        Icon::Search,
        Icon::Info,
        Icon::Palette,
        Icon::Pencil,
        Icon::Eye,
    ];
}

/// Logical drawing grid. Every icon is designed on 16×16 units.
const GRID: f32 = 16.0;

/// Rasterised texture size in texels.
const TEXTURE_SIZE: usize = 32;

/// Samples per texture pixel on each axis; the mean gives antialiasing.
const SUPERSAMPLE: usize = 3;

/// Edge displayed at [`ICON_SIZE`] logical points.
const ICON_SIZE: f32 = 16.0;

/// A pixel of a menu icon.
#[derive(Clone, Copy, Debug)]
enum Prim {
    Rect {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    },
    Frame {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        width: f32,
    },
    Disc {
        cx: f32,
        cy: f32,
        r: f32,
    },
    Ring {
        cx: f32,
        cy: f32,
        r: f32,
        width: f32,
    },
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        width: f32,
    },
    Poly {
        points: &'static [(f32, f32)],
        closed: bool,
        filled: bool,
        width: f32,
    },
    Arc {
        cx: f32,
        cy: f32,
        r: f32,
        width: f32,
        start: f32,
        end: f32,
    },
    Crescent {
        cx: f32,
        cy: f32,
        r: f32,
        cut_cx: f32,
        cut_cy: f32,
        cut_r: f32,
    },
    /// Overlap of two equal circles.
    Lens {
        cx1: f32,
        cy1: f32,
        cx2: f32,
        cy2: f32,
        r: f32,
    },
}

const fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Prim {
    Prim::Rect { x0, y0, x1, y1 }
}

const fn frame(x0: f32, y0: f32, x1: f32, y1: f32, width: f32) -> Prim {
    Prim::Frame {
        x0,
        y0,
        x1,
        y1,
        width,
    }
}

const fn disc(cx: f32, cy: f32, r: f32) -> Prim {
    Prim::Disc { cx, cy, r }
}

const fn ring(cx: f32, cy: f32, r: f32, width: f32) -> Prim {
    Prim::Ring { cx, cy, r, width }
}

const fn line(x0: f32, y0: f32, x1: f32, y1: f32, width: f32) -> Prim {
    Prim::Line {
        x0,
        y0,
        x1,
        y1,
        width,
    }
}

const fn poly(points: &'static [(f32, f32)], closed: bool, filled: bool, width: f32) -> Prim {
    Prim::Poly {
        points,
        closed,
        filled,
        width,
    }
}

const fn arc(cx: f32, cy: f32, r: f32, width: f32, start: f32, end: f32) -> Prim {
    Prim::Arc {
        cx,
        cy,
        r,
        width,
        start,
        end,
    }
}

// Point sets used by more than one icon.

static FOLDER: &[(f32, f32)] = &[
    (1.6, 4.4),
    (5.6, 4.4),
    (6.9, 6.2),
    (14.4, 6.2),
    (14.4, 13.6),
    (1.6, 13.6),
];
static MOUNTAIN: &[(f32, f32)] = &[
    (2.2, 11.6),
    (6.0, 7.6),
    (9.0, 10.6),
    (11.2, 8.4),
    (13.8, 11.6),
];
static TRAY: &[(f32, f32)] = &[(2.0, 9.8), (2.0, 14.0), (14.0, 14.0), (14.0, 9.8)];
static ARROW_DOWN: &[(f32, f32)] = &[(5.1, 5.9), (10.9, 5.9), (8.0, 9.6)];
static ARROW_UP: &[(f32, f32)] = &[(5.1, 5.9), (10.9, 5.9), (8.0, 2.2)];
static ARROW_LEFT: &[(f32, f32)] = &[(6.2, 5.1), (6.2, 10.9), (2.5, 8.0)];
static ARROW_RIGHT: &[(f32, f32)] = &[(9.8, 5.1), (9.8, 10.9), (13.5, 8.0)];
static CHECK: &[(f32, f32)] = &[(2.6, 8.4), (6.3, 12.1), (13.5, 3.7)];
static CURSOR: &[(f32, f32)] = &[
    (3.4, 1.5),
    (12.7, 9.1),
    (8.8, 9.7),
    (11.1, 14.3),
    (8.8, 15.1),
    (6.5, 10.4),
    (3.4, 12.6),
];
static PENTAGON: &[(f32, f32)] = &[
    (8.0, 2.2),
    (13.9, 6.7),
    (11.6, 13.7),
    (4.4, 13.7),
    (2.1, 6.7),
];
static STAR: &[(f32, f32)] = &[
    (8.0, 1.8),
    (9.7, 6.05),
    (14.3, 6.35),
    (10.8, 9.3),
    (11.9, 13.75),
    (8.0, 11.3),
    (4.1, 13.75),
    (5.2, 9.3),
    (1.7, 6.35),
    (6.3, 6.05),
];
static ZIGZAG: &[(f32, f32)] = &[(2.4, 12.4), (6.0, 4.8), (10.0, 10.2), (13.6, 3.4)];
static SQUIGGLE: &[(f32, f32)] = &[
    (1.8, 11.6),
    (3.6, 6.4),
    (5.4, 10.4),
    (7.4, 5.2),
    (9.4, 9.6),
    (11.4, 4.6),
    (13.4, 8.6),
    (14.2, 6.4),
];
static BEZIER: &[(f32, f32)] = &[
    (2.0, 12.0),
    (2.9, 9.1),
    (3.9, 6.6),
    (5.2, 4.9),
    (7.0, 4.3),
    (9.0, 4.9),
    (10.7, 6.7),
    (12.5, 9.5),
    (14.0, 12.0),
];
static SKEW: &[(f32, f32)] = &[(2.6, 13.6), (6.0, 2.4), (13.4, 2.4), (10.0, 13.6)];
static MAGNET: &[(f32, f32)] = &[
    (3.2, 2.4),
    (3.2, 9.0),
    (5.4, 12.6),
    (10.6, 12.6),
    (12.8, 9.0),
    (12.8, 2.4),
];
static HEAD: &[(f32, f32)] = &[(2.4, 6.2), (7.6, 2.4), (7.6, 10.0)];
static DROP: &[(f32, f32)] = &[(8.0, 2.0), (3.0, 9.0), (13.0, 9.0)];
static OPEN_PATH: &[(f32, f32)] = &[(2.6, 6.0), (7.0, 2.6), (13.4, 7.0), (9.2, 13.4)];
static PENCIL: &[(f32, f32)] = &[
    (2.2, 13.8),
    (3.0, 10.2),
    (10.6, 2.6),
    (13.4, 5.4),
    (5.8, 13.0),
];

fn prims(icon: Icon) -> Vec<Prim> {
    match icon {
        Icon::File | Icon::Open => vec![poly(FOLDER, true, false, 1.3)],
        Icon::Edit | Icon::Pencil => vec![
            poly(PENCIL, true, false, 1.3),
            line(3.0, 10.2, 5.8, 13.0, 1.0),
        ],
        Icon::Draw | Icon::Freehand => vec![poly(SQUIGGLE, false, false, 1.6)],
        Icon::Operations | Icon::Gear => vec![
            ring(8.0, 8.0, 4.0, 1.5),
            line(12.8, 8.0, 14.4, 8.0, 1.4),
            line(8.0, 3.2, 8.0, 1.6, 1.4),
            line(3.2, 8.0, 1.6, 8.0, 1.4),
            line(8.0, 12.8, 8.0, 14.4, 1.4),
            line(11.4, 4.6, 12.5, 3.5, 1.4),
            line(4.6, 4.6, 3.5, 3.5, 1.4),
            line(4.6, 11.4, 3.5, 12.5, 1.4),
            line(11.4, 11.4, 12.5, 12.5, 1.4),
        ],
        Icon::Materials | Icon::Palette => vec![
            ring(8.0, 8.0, 5.8, 1.3),
            disc(5.6, 6.2, 0.9),
            disc(9.0, 4.8, 0.9),
            disc(11.2, 8.0, 0.9),
            disc(5.8, 10.4, 0.9),
        ],
        Icon::Arrange | Icon::Layers => vec![
            poly(
                &[(2.0, 5.2), (8.0, 2.2), (14.0, 5.2), (8.0, 8.2)],
                true,
                false,
                1.2,
            ),
            poly(&[(2.0, 8.4), (8.0, 11.4), (14.0, 8.4)], false, false, 1.2),
            poly(&[(2.0, 11.4), (8.0, 14.4), (14.0, 11.4)], false, false, 1.2),
        ],
        Icon::Align | Icon::AlignLeft => vec![
            line(2.4, 2.0, 2.4, 14.0, 1.5),
            rect(4.4, 3.4, 12.2, 6.6),
            rect(4.4, 9.4, 9.6, 12.6),
        ],
        Icon::AlignCenterH => vec![
            line(8.0, 2.0, 8.0, 14.0, 1.5),
            rect(3.0, 3.4, 13.0, 6.6),
            rect(5.0, 9.4, 11.0, 12.6),
        ],
        Icon::AlignRight => vec![
            line(13.6, 2.0, 13.6, 14.0, 1.5),
            rect(3.8, 3.4, 11.6, 6.6),
            rect(6.4, 9.4, 11.6, 12.6),
        ],
        Icon::AlignTop => vec![
            line(2.0, 2.4, 14.0, 2.4, 1.5),
            rect(3.4, 4.4, 6.6, 12.2),
            rect(9.4, 4.4, 12.6, 9.6),
        ],
        Icon::AlignCenterV => vec![
            line(2.0, 8.0, 14.0, 8.0, 1.5),
            rect(3.4, 3.0, 6.6, 13.0),
            rect(9.4, 5.0, 12.6, 11.0),
        ],
        Icon::AlignBottom => vec![
            line(2.0, 13.6, 14.0, 13.6, 1.5),
            rect(3.4, 3.8, 6.6, 11.6),
            rect(9.4, 6.4, 12.6, 11.6),
        ],
        Icon::Profiles | Icon::Book => vec![
            frame(1.8, 2.0, 7.2, 14.0, 1.3),
            frame(8.8, 2.0, 14.2, 14.0, 1.3),
            line(3.4, 4.6, 5.8, 4.6, 1.0),
            line(3.4, 7.0, 5.8, 7.0, 1.0),
            line(10.2, 4.6, 12.6, 4.6, 1.0),
            line(10.2, 7.0, 12.6, 7.0, 1.0),
        ],
        Icon::View | Icon::Eye => vec![ring(8.0, 8.0, 4.4, 1.3), disc(8.0, 8.0, 2.0)],
        Icon::Help => vec![
            arc(
                8.0,
                5.8,
                2.7,
                1.5,
                -0.55 * std::f32::consts::PI,
                1.05 * std::f32::consts::PI,
            ),
            line(8.0, 8.4, 8.0, 10.2, 1.5),
            disc(8.0, 12.6, 0.9),
        ],
        Icon::New => vec![
            frame(3.0, 1.6, 12.6, 14.4, 1.3),
            line(8.0, 4.6, 8.0, 10.0, 1.4),
            line(5.3, 7.3, 10.7, 7.3, 1.4),
        ],
        Icon::Save => vec![
            frame(2.0, 2.0, 14.0, 14.0, 1.3),
            rect(5.0, 2.0, 11.0, 6.6),
            frame(4.8, 9.6, 11.2, 13.3, 1.0),
        ],
        Icon::Import => vec![
            line(8.0, 1.8, 8.0, 8.4, 1.5),
            poly(ARROW_DOWN, true, true, 0.0),
            poly(TRAY, false, false, 1.4),
        ],
        Icon::Export => vec![
            line(8.0, 9.2, 8.0, 3.0, 1.5),
            poly(ARROW_UP, true, true, 0.0),
            poly(TRAY, false, false, 1.4),
        ],
        Icon::Page => vec![
            frame(3.0, 1.6, 12.6, 14.4, 1.3),
            line(5.4, 5.2, 10.2, 5.2, 1.1),
            line(5.4, 7.6, 10.2, 7.6, 1.1),
            line(5.4, 10.0, 8.4, 10.0, 1.1),
        ],
        Icon::Image => vec![
            frame(1.8, 3.2, 14.2, 12.8, 1.3),
            disc(5.2, 6.1, 1.0),
            poly(MOUNTAIN, false, false, 1.3),
        ],
        Icon::Recover => vec![
            arc(8.0, 8.0, 5.0, 1.5, 0.0, 1.5 * std::f32::consts::PI),
            poly(&[(7.2, 1.4), (7.2, 4.6), (10.8, 3.0)], true, true, 0.0),
        ],
        Icon::Exit => vec![
            frame(2.6, 1.6, 9.0, 14.4, 1.3),
            line(9.0, 8.0, 13.6, 8.0, 1.5),
            poly(ARROW_RIGHT, true, true, 0.0),
        ],
        Icon::Undo => vec![
            arc(
                8.0,
                8.8,
                4.8,
                1.5,
                std::f32::consts::PI,
                2.0 * std::f32::consts::PI,
            ),
            poly(&[(6.2, 9.0), (2.4, 9.0), (4.4, 5.6)], true, true, 0.0),
        ],
        Icon::Redo => vec![
            arc(
                8.0,
                8.8,
                4.8,
                1.5,
                std::f32::consts::PI,
                2.0 * std::f32::consts::PI,
            ),
            poly(&[(9.8, 9.0), (13.6, 9.0), (11.6, 5.6)], true, true, 0.0),
        ],
        Icon::SelectAll => vec![frame(2.6, 2.6, 13.4, 13.4, 1.4), rect(5.2, 5.2, 10.8, 10.8)],
        Icon::Cut => vec![
            ring(4.6, 11.2, 2.0, 1.3),
            ring(4.6, 4.8, 2.0, 1.3),
            line(6.2, 10.4, 13.2, 3.2, 1.4),
            line(6.2, 5.6, 13.2, 12.8, 1.4),
        ],
        Icon::Copy => vec![
            frame(2.0, 2.0, 10.0, 10.0, 1.3),
            frame(6.0, 6.0, 14.0, 14.0, 1.3),
        ],
        Icon::Paste => vec![
            frame(2.8, 3.4, 13.2, 14.4, 1.3),
            frame(5.6, 1.6, 10.4, 4.2, 1.2),
            line(5.2, 7.2, 10.8, 7.2, 1.1),
            line(5.2, 9.6, 10.8, 9.6, 1.1),
        ],
        Icon::Duplicate => vec![
            frame(2.0, 2.0, 9.4, 9.4, 1.2),
            frame(6.6, 6.6, 14.0, 14.0, 1.2),
            line(11.7, 8.4, 11.7, 12.2, 1.2),
            line(9.8, 10.3, 13.6, 10.3, 1.2),
        ],
        Icon::Delete => vec![
            line(2.4, 3.6, 13.6, 3.6, 1.5),
            line(6.0, 1.7, 10.0, 1.7, 1.3),
            poly(
                &[(4.2, 4.6), (5.0, 14.0), (11.0, 14.0), (11.8, 4.6)],
                false,
                false,
                1.3,
            ),
            line(6.4, 6.6, 6.7, 11.8, 1.0),
            line(9.6, 6.6, 9.3, 11.8, 1.0),
        ],
        Icon::Invert => vec![
            frame(2.6, 2.6, 13.4, 13.4, 1.3),
            poly(&[(3.6, 12.4), (12.4, 3.6), (12.4, 12.4)], true, true, 0.0),
        ],
        Icon::Lock => vec![
            frame(4.0, 7.0, 12.0, 13.6, 1.4),
            arc(
                8.0,
                7.0,
                2.7,
                1.4,
                std::f32::consts::PI,
                2.0 * std::f32::consts::PI,
            ),
            disc(8.0, 10.2, 0.9),
            line(8.0, 10.8, 8.0, 12.2, 1.1),
        ],
        Icon::Unlock => vec![
            frame(4.0, 7.0, 12.0, 13.6, 1.4),
            arc(
                10.2,
                7.0,
                2.7,
                1.4,
                std::f32::consts::PI,
                1.72 * std::f32::consts::PI,
            ),
            disc(8.0, 10.2, 0.9),
            line(8.0, 10.8, 8.0, 12.2, 1.1),
        ],
        Icon::Preferences => vec![
            line(2.0, 4.2, 14.0, 4.2, 1.3),
            line(2.0, 8.0, 14.0, 8.0, 1.3),
            line(2.0, 11.8, 14.0, 11.8, 1.3),
            disc(10.0, 4.2, 1.7),
            disc(5.4, 8.0, 1.7),
            disc(11.0, 11.8, 1.7),
        ],
        Icon::Group => vec![
            frame(1.8, 1.8, 14.2, 14.2, 1.1),
            rect(3.8, 3.8, 7.6, 7.6),
            rect(8.4, 8.4, 12.2, 12.2),
        ],
        Icon::Ungroup => vec![
            frame(1.8, 1.8, 7.6, 7.6, 1.2),
            frame(8.4, 8.4, 14.2, 14.2, 1.2),
        ],
        Icon::Select => vec![poly(CURSOR, true, true, 0.0)],
        Icon::Line => vec![line(3.2, 12.8, 12.8, 3.2, 1.8)],
        Icon::Rectangle => vec![frame(2.6, 3.4, 13.4, 12.6, 1.5)],
        Icon::Ellipse => vec![ring(8.0, 8.0, 5.7, 1.5)],
        Icon::Polygon => vec![poly(PENTAGON, true, false, 1.4)],
        Icon::Star => vec![poly(STAR, true, false, 1.3)],
        Icon::Text => vec![
            line(3.2, 3.0, 12.8, 3.0, 1.8),
            line(8.0, 3.0, 8.0, 13.4, 1.8),
        ],
        Icon::Measure | Icon::Rulers => vec![
            frame(1.4, 5.4, 14.6, 10.6, 1.3),
            line(4.6, 5.4, 4.6, 7.8, 1.1),
            line(7.6, 5.4, 7.6, 8.6, 1.1),
            line(10.6, 5.4, 10.6, 7.8, 1.1),
        ],
        Icon::Bezier => vec![poly(BEZIER, false, false, 1.6)],
        Icon::Pan => vec![
            line(8.0, 2.0, 8.0, 14.0, 1.3),
            line(2.0, 8.0, 14.0, 8.0, 1.3),
            poly(&[(6.2, 4.4), (9.8, 4.4), (8.0, 1.8)], true, true, 0.0),
            poly(&[(6.2, 11.6), (9.8, 11.6), (8.0, 14.2)], true, true, 0.0),
            poly(&[(4.4, 6.2), (4.4, 9.8), (1.8, 8.0)], true, true, 0.0),
            poly(&[(11.6, 6.2), (11.6, 9.8), (14.2, 8.0)], true, true, 0.0),
        ],
        Icon::Nodes => vec![
            poly(ZIGZAG, false, false, 1.4),
            rect(1.5, 11.5, 3.3, 13.3),
            rect(5.1, 3.9, 6.9, 5.7),
            rect(9.1, 9.3, 10.9, 11.1),
            rect(12.7, 2.5, 14.5, 4.3),
        ],
        Icon::Trim => vec![
            line(2.6, 8.0, 6.0, 8.0, 1.6),
            line(10.0, 8.0, 13.4, 8.0, 1.6),
            line(7.2, 5.4, 8.8, 10.6, 1.2),
            line(8.8, 5.4, 7.2, 10.6, 1.2),
        ],
        Icon::Join => vec![
            line(2.4, 8.0, 6.4, 8.0, 1.6),
            line(9.6, 8.0, 13.6, 8.0, 1.6),
            disc(8.0, 8.0, 1.5),
        ],
        Icon::Plus => vec![
            ring(8.0, 8.0, 5.6, 1.4),
            line(8.0, 4.8, 8.0, 11.2, 1.5),
            line(4.8, 8.0, 11.2, 8.0, 1.5),
        ],
        Icon::Compare => vec![
            rect(2.4, 7.6, 5.4, 13.4),
            rect(6.6, 3.6, 9.6, 13.4),
            rect(10.8, 9.6, 13.8, 13.4),
            line(1.6, 14.2, 14.4, 14.2, 1.2),
        ],
        Icon::Play => vec![poly(
            &[(4.0, 2.6), (13.4, 8.0), (4.0, 13.4)],
            true,
            true,
            0.0,
        )],
        Icon::Clock => vec![
            ring(8.0, 8.0, 5.7, 1.5),
            line(8.0, 8.0, 8.0, 4.6, 1.4),
            line(8.0, 8.0, 10.8, 9.3, 1.4),
        ],
        Icon::Check => vec![poly(CHECK, false, false, 1.8)],
        Icon::ToFront => vec![frame(1.8, 6.2, 9.2, 13.8, 1.2), rect(6.6, 2.2, 14.0, 9.8)],
        Icon::ToBack => vec![rect(1.8, 6.2, 9.2, 13.8), frame(6.6, 2.2, 14.0, 9.8, 1.2)],
        Icon::Center => vec![
            frame(2.0, 2.0, 14.0, 14.0, 1.2),
            line(8.0, 4.8, 8.0, 11.2, 1.3),
            line(4.8, 8.0, 11.2, 8.0, 1.3),
            disc(8.0, 8.0, 1.0),
        ],
        Icon::DistributeH => vec![
            rect(2.0, 2.6, 4.2, 13.4),
            rect(6.9, 2.6, 9.1, 13.4),
            rect(11.8, 2.6, 14.0, 13.4),
        ],
        Icon::DistributeV => vec![
            rect(2.6, 2.0, 13.4, 4.2),
            rect(2.6, 6.9, 13.4, 9.1),
            rect(2.6, 11.8, 13.4, 14.0),
        ],
        Icon::RotateLeft => vec![
            arc(8.0, 8.0, 5.0, 1.5, 0.0, 1.5 * std::f32::consts::PI),
            poly(&[(9.0, 1.4), (9.0, 4.6), (5.4, 3.0)], true, true, 0.0),
        ],
        Icon::RotateRight => vec![
            arc(8.0, 8.0, 5.0, 1.5, 0.0, 1.5 * std::f32::consts::PI),
            poly(&[(7.0, 1.4), (7.0, 4.6), (10.6, 3.0)], true, true, 0.0),
        ],
        Icon::FlipH => vec![
            line(8.0, 1.6, 8.0, 14.4, 1.1),
            poly(&[(6.4, 3.6), (2.2, 8.0), (6.4, 12.4)], true, true, 0.0),
            poly(&[(9.6, 3.6), (13.8, 8.0), (9.6, 12.4)], true, true, 0.0),
        ],
        Icon::FlipV => vec![
            line(1.6, 8.0, 14.4, 8.0, 1.1),
            poly(&[(3.6, 6.4), (8.0, 2.2), (12.4, 6.4)], true, true, 0.0),
            poly(&[(3.6, 9.6), (8.0, 13.8), (12.4, 9.6)], true, true, 0.0),
        ],
        Icon::ScaleUp => vec![
            line(8.0, 8.0, 13.2, 2.8, 1.5),
            poly(&[(10.2, 2.0), (14.0, 2.0), (14.0, 5.8)], true, true, 0.0),
            line(8.0, 8.0, 2.8, 13.2, 1.5),
            poly(&[(2.0, 10.2), (2.0, 14.0), (5.8, 14.0)], true, true, 0.0),
        ],
        Icon::ScaleDown => vec![
            frame(1.8, 1.8, 14.2, 14.2, 1.2),
            line(8.0, 8.0, 5.0, 11.0, 1.4),
            poly(&[(4.0, 8.6), (4.0, 12.0), (7.4, 12.0)], true, true, 0.0),
            line(8.0, 8.0, 11.0, 5.0, 1.4),
            poly(&[(12.0, 7.4), (12.0, 4.0), (8.6, 4.0)], true, true, 0.0),
        ],
        Icon::TextOutline => vec![
            frame(1.6, 1.6, 14.4, 14.4, 1.0),
            line(4.4, 4.2, 11.6, 4.2, 1.7),
            line(8.0, 4.2, 8.0, 12.4, 1.7),
        ],
        Icon::TextOnPath => vec![
            line(5.4, 2.4, 10.6, 2.4, 1.5),
            line(8.0, 2.4, 8.0, 8.6, 1.5),
            arc(
                8.0,
                7.4,
                5.4,
                1.3,
                0.16 * std::f32::consts::PI,
                0.84 * std::f32::consts::PI,
            ),
        ],
        Icon::TransformCenter => vec![
            ring(8.0, 8.0, 3.1, 1.2),
            line(8.0, 1.6, 8.0, 4.6, 1.3),
            line(8.0, 11.4, 8.0, 14.4, 1.3),
            line(1.6, 8.0, 4.6, 8.0, 1.3),
            line(11.4, 8.0, 14.4, 8.0, 1.3),
        ],
        Icon::Skew => vec![poly(SKEW, true, false, 1.4)],
        Icon::GridArray => vec![
            rect(2.2, 2.2, 4.6, 4.6),
            rect(6.8, 2.2, 9.2, 4.6),
            rect(11.4, 2.2, 13.8, 4.6),
            rect(2.2, 6.8, 4.6, 9.2),
            rect(6.8, 6.8, 9.2, 9.2),
            rect(11.4, 6.8, 13.8, 9.2),
            rect(2.2, 11.4, 4.6, 13.8),
            rect(6.8, 11.4, 9.2, 13.8),
            rect(11.4, 11.4, 13.8, 13.8),
        ],
        Icon::ClosePath => vec![
            poly(OPEN_PATH, false, false, 1.4),
            line(9.2, 13.4, 2.6, 6.0, 1.4),
            disc(2.6, 6.0, 1.3),
        ],
        Icon::Simplify => vec![
            poly(ZIGZAG, false, false, 1.3),
            rect(5.1, 3.9, 6.9, 5.7),
            rect(9.1, 9.3, 10.9, 11.1),
        ],
        Icon::Repeat => vec![
            arc(
                8.0,
                8.0,
                4.6,
                1.4,
                -0.45 * std::f32::consts::PI,
                0.55 * std::f32::consts::PI,
            ),
            poly(&[(9.4, 2.6), (13.4, 5.2), (9.4, 7.4)], true, true, 0.0),
            arc(
                8.0,
                8.0,
                4.6,
                1.4,
                0.55 * std::f32::consts::PI,
                1.55 * std::f32::consts::PI,
            ),
            poly(&[(6.6, 13.4), (2.6, 10.8), (6.6, 8.6)], true, true, 0.0),
        ],
        Icon::Nest => vec![
            frame(1.8, 1.8, 14.2, 14.2, 1.2),
            frame(4.8, 4.8, 11.2, 11.2, 1.2),
            rect(7.2, 7.2, 8.8, 8.8),
        ],
        Icon::FindDuplicates => vec![
            frame(2.0, 2.0, 9.4, 9.4, 1.2),
            frame(6.6, 6.6, 14.0, 14.0, 1.2),
            disc(8.0, 8.0, 1.2),
        ],
        Icon::Intersections => vec![
            line(2.4, 2.4, 13.6, 13.6, 1.4),
            line(13.6, 2.4, 2.4, 13.6, 1.4),
            disc(8.0, 8.0, 1.6),
        ],
        Icon::Boolean => vec![ring(6.2, 8.0, 4.3, 1.4), ring(9.8, 8.0, 4.3, 1.4)],
        Icon::BooleanUnion => vec![disc(6.2, 8.0, 4.3), disc(9.8, 8.0, 4.3)],
        Icon::BooleanIntersection => vec![
            ring(6.2, 8.0, 4.3, 1.4),
            ring(9.8, 8.0, 4.3, 1.4),
            Prim::Lens {
                cx1: 6.2,
                cy1: 8.0,
                cx2: 9.8,
                cy2: 8.0,
                r: 4.3,
            },
        ],
        Icon::BooleanDifference => vec![
            ring(6.2, 8.0, 4.3, 1.4),
            ring(9.8, 8.0, 4.3, 1.4),
            Prim::Crescent {
                cx: 6.2,
                cy: 8.0,
                r: 4.3,
                cut_cx: 9.8,
                cut_cy: 8.0,
                cut_r: 4.3,
            },
        ],
        Icon::BooleanExclusion => vec![
            ring(6.2, 8.0, 4.3, 1.4),
            ring(9.8, 8.0, 4.3, 1.4),
            Prim::Crescent {
                cx: 6.2,
                cy: 8.0,
                r: 4.3,
                cut_cx: 9.8,
                cut_cy: 8.0,
                cut_r: 4.3,
            },
            Prim::Crescent {
                cx: 9.8,
                cy: 8.0,
                r: 4.3,
                cut_cx: 6.2,
                cut_cy: 8.0,
                cut_r: 4.3,
            },
        ],
        Icon::OutlineStroke => vec![frame(1.8, 8.6, 14.2, 11.4, 0.9)],
        Icon::Offset => vec![
            frame(2.0, 2.0, 14.0, 14.0, 1.1),
            frame(5.2, 5.2, 10.8, 10.8, 1.1),
        ],
        Icon::ZoomIn => vec![
            ring(7.0, 7.0, 4.4, 1.5),
            line(10.2, 10.2, 14.0, 14.0, 1.8),
            line(4.8, 7.0, 9.2, 7.0, 1.3),
            line(7.0, 4.8, 7.0, 9.2, 1.3),
        ],
        Icon::ZoomOut => vec![
            ring(7.0, 7.0, 4.4, 1.5),
            line(10.2, 10.2, 14.0, 14.0, 1.8),
            line(4.8, 7.0, 9.2, 7.0, 1.3),
        ],
        Icon::ZoomFit => vec![
            frame(2.4, 2.4, 13.6, 13.6, 1.3),
            line(5.2, 5.2, 10.8, 5.2, 1.1),
            line(5.2, 10.8, 10.8, 10.8, 1.1),
            line(5.2, 5.2, 5.2, 10.8, 1.1),
            line(10.8, 5.2, 10.8, 10.8, 1.1),
        ],
        Icon::ZoomSelection => vec![
            frame(2.0, 2.0, 14.0, 14.0, 1.1),
            ring(7.0, 7.0, 3.0, 1.3),
            line(9.2, 9.2, 12.6, 12.6, 1.5),
        ],
        Icon::Grid => vec![
            frame(2.0, 2.0, 14.0, 14.0, 1.2),
            line(6.0, 2.0, 6.0, 14.0, 1.0),
            line(10.0, 2.0, 10.0, 14.0, 1.0),
            line(2.0, 6.0, 14.0, 6.0, 1.0),
            line(2.0, 10.0, 14.0, 10.0, 1.0),
        ],
        Icon::Guides => vec![
            line(8.0, 1.6, 8.0, 14.4, 1.2),
            frame(6.0, 6.0, 10.0, 10.0, 1.3),
        ],
        Icon::Snap => vec![
            poly(MAGNET, false, false, 1.7),
            line(3.2, 6.2, 6.2, 6.2, 1.7),
            line(9.8, 6.2, 12.8, 6.2, 1.7),
        ],
        Icon::SnapCenter => vec![ring(8.0, 8.0, 4.6, 1.3), disc(8.0, 8.0, 1.4)],
        Icon::SnapMidpoint => vec![line(2.2, 8.0, 13.8, 8.0, 1.5), rect(6.4, 6.4, 9.6, 9.6)],
        Icon::SnapLimits => vec![
            frame(2.6, 2.6, 13.4, 13.4, 1.1),
            disc(2.6, 2.6, 1.4),
            disc(13.4, 2.6, 1.4),
            disc(2.6, 13.4, 1.4),
            disc(13.4, 13.4, 1.4),
        ],
        Icon::Toolpaths => vec![poly(ZIGZAG, false, false, 1.5), disc(13.6, 3.4, 1.2)],
        Icon::Panels => vec![
            frame(1.8, 2.2, 14.2, 13.8, 1.2),
            line(6.2, 2.2, 6.2, 13.8, 1.1),
        ],
        Icon::DarkMode => vec![Prim::Crescent {
            cx: 8.6,
            cy: 8.0,
            r: 5.6,
            cut_cx: 11.4,
            cut_cy: 5.6,
            cut_r: 4.9,
        }],
        Icon::Search => vec![ring(6.8, 6.8, 4.6, 1.3), line(10.2, 10.2, 14.0, 14.0, 1.5)],
        Icon::Info => vec![
            ring(8.0, 8.0, 5.8, 1.4),
            disc(8.0, 4.8, 0.9),
            line(8.0, 6.8, 8.0, 11.6, 1.4),
        ],
    }
}

impl Icon {
    /// Rasterise the icon to a 32×32 white texture.
    ///
    /// Exposed so tests and previews can inspect the drawing without a GPU.
    #[allow(clippy::cast_precision_loss)]
    pub fn raster(self) -> ColorImage {
        let prims = prims(self);
        let ss = SUPERSAMPLE;
        let samples = TEXTURE_SIZE * ss;
        let mut pixels = Vec::with_capacity(TEXTURE_SIZE * TEXTURE_SIZE);
        for py in 0..TEXTURE_SIZE {
            for px in 0..TEXTURE_SIZE {
                let mut hits = 0u32;
                for sy in 0..ss {
                    for sx in 0..ss {
                        let x = (px * ss + sx) as f32 / samples as f32 * GRID;
                        let y = (py * ss + sy) as f32 / samples as f32 * GRID;
                        if prims.iter().any(|prim| inside(prim, x, y)) {
                            hits += 1;
                        }
                    }
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let alpha = (hits * 255 / (ss * ss) as u32) as u8;
                pixels.push(Color32::from_rgba_unmultiplied(255, 255, 255, alpha));
            }
        }
        ColorImage::new([TEXTURE_SIZE, TEXTURE_SIZE], pixels)
    }
}

fn inside(prim: &Prim, x: f32, y: f32) -> bool {
    match *prim {
        Prim::Rect { x0, y0, x1, y1 } => x >= x0 && x <= x1 && y >= y0 && y <= y1,
        Prim::Frame {
            x0,
            y0,
            x1,
            y1,
            width,
        } => {
            let outer = x >= x0 && x <= x1 && y >= y0 && y <= y1;
            let inner = x > x0 + width && x < x1 - width && y > y0 + width && y < y1 - width;
            outer && !inner
        }
        Prim::Disc { cx, cy, r } => (x - cx).hypot(y - cy) <= r,
        Prim::Ring { cx, cy, r, width } => ((x - cx).hypot(y - cy) - r).abs() <= width / 2.0,
        Prim::Line {
            x0,
            y0,
            x1,
            y1,
            width,
        } => distance_to_segment(x, y, x0, y0, x1, y1) <= width / 2.0,
        Prim::Poly {
            points,
            closed,
            filled,
            width,
        } => {
            if filled && point_in_polygon(x, y, points) {
                return true;
            }
            let count = points.len();
            (0..count).any(|i| {
                let next = i + 1;
                if next == count {
                    if closed {
                        distance_to_segment(
                            x,
                            y,
                            points[i].0,
                            points[i].1,
                            points[0].0,
                            points[0].1,
                        ) <= width / 2.0
                    } else {
                        false
                    }
                } else {
                    distance_to_segment(
                        x,
                        y,
                        points[i].0,
                        points[i].1,
                        points[next].0,
                        points[next].1,
                    ) <= width / 2.0
                }
            })
        }
        Prim::Arc {
            cx,
            cy,
            r,
            width,
            start,
            end,
        } => {
            let dist = (x - cx).hypot(y - cy);
            if (dist - r).abs() > width / 2.0 {
                return false;
            }
            let angle = (y - cy).atan2(x - cx);
            angle_in_arc(angle, start, end)
        }
        Prim::Crescent {
            cx,
            cy,
            r,
            cut_cx,
            cut_cy,
            cut_r,
        } => {
            let inside_outer = (x - cx).hypot(y - cy) <= r;
            let inside_cut = (x - cut_cx).hypot(y - cut_cy) <= cut_r;
            inside_outer && !inside_cut
        }
        Prim::Lens {
            cx1,
            cy1,
            cx2,
            cy2,
            r,
        } => (x - cx1).hypot(y - cy1) <= r && (x - cx2).hypot(y - cy2) <= r,
    }
}

fn angle_in_arc(angle: f32, start: f32, end: f32) -> bool {
    let tau = std::f32::consts::TAU;
    let norm = |value: f32| {
        let mut value = value % tau;
        if value < 0.0 {
            value += tau;
        }
        value
    };
    let angle = norm(angle);
    let start = norm(start);
    let end = norm(end);
    if start <= end {
        angle >= start && angle <= end
    } else {
        angle >= start || angle <= end
    }
}

fn point_in_polygon(x: f32, y: f32, points: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let count = points.len();
    let mut j = count - 1;
    for i in 0..count {
        let (xi, yi) = points[i];
        let (xj, yj) = points[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn distance_to_segment(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> f32 {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let length_sq = dx * dx + dy * dy;
    if length_sq <= f32::EPSILON {
        return (x - x0).hypot(y - y0);
    }
    let t = (((x - x0) * dx + (y - y0) * dy) / length_sq).clamp(0.0, 1.0);
    (x - (x0 + t * dx)).hypot(y - (y0 + t * dy))
}

/// Texture for `icon`, created on first use and cached on the egui context.
fn texture(ctx: &Context, icon: Icon) -> TextureHandle {
    let id = egui::Id::new(("beamwright_menu_icon", icon));
    if let Some(handle) = ctx.data(|data| data.get_temp::<TextureHandle>(id)) {
        return handle;
    }
    let handle = ctx.load_texture(
        format!("beamwright_icon_{icon:?}"),
        icon.raster(),
        TextureOptions::LINEAR,
    );
    ctx.data_mut(|data| data.insert_temp(id, handle.clone()));
    handle
}

/// The icon as an egui image, sized for a menu row.
pub fn image(ctx: &Context, icon: Icon) -> Image<'static> {
    image_at(ctx, icon, ICON_SIZE)
}

/// The icon as an egui image at an explicit size, for toolbars.
pub fn image_at(ctx: &Context, icon: Icon, size: f32) -> Image<'static> {
    let handle = texture(ctx, icon);
    Image::new(egui::load::SizedTexture::from_handle(&handle)).fit_to_exact_size(Vec2::splat(size))
}

/// One-line description shown when hovering an entry.
pub fn tooltip(label: &str) -> Option<&'static str> {
    let text = match label {
        "New" => "Start an empty project",
        "Open..." => "Open a .bwp project or an .svg drawing",
        "Save" => "Save the project, keeping a backup of the previous version",
        "Save as..." => "Save the project under another name",
        "Import SVG or DXF..." => "Import an SVG, DXF or raster image",
        "Export SVG..." => "Export the document as SVG",
        "Export DXF..." => "Export the document as DXF",
        "Export PNG..." => "Export a raster preview of the document",
        "Export G-code..." => "Generate and save the laser job",
        "Export selection" => "Export only the selected objects",
        "As SVG..." => "Export the selection as SVG",
        "As DXF..." => "Export the selection as DXF",
        "As G-code..." => "Export the selection as a laser job",
        "Document properties..." => "Edit the name, work area and unit",
        "Collect images..." => "Copy referenced images into the project folder",
        "Recover project..." => "Open the backup of a project",
        "Machine profiles..." => "Create and apply saved work areas",
        "Exit" => "Close BeamWright",
        "Select all" => "Select every visible, unlocked object (Ctrl+A)",
        "Cut" => "Cut the selection to the internal clipboard (Ctrl+X)",
        "Copy" => "Copy the selection to the internal clipboard (Ctrl+C)",
        "Paste" => "Paste the clipboard into the document (Ctrl+V)",
        "Duplicate" => "Duplicate the selected objects (Ctrl+D)",
        "Delete" => "Delete the selected objects (Del or Backspace)",
        "Invert selection" => "Select everything that is not selected",
        "Lock" => "Prevent the selected objects from being edited",
        "Unlock" => "Allow the selected objects to be edited",
        "Preferences..." => "Edit the application preferences",
        "Group" => "Group the selected objects (Ctrl+G)",
        "Ungroup" => "Dissolve the selected group (Ctrl+Shift+G)",
        "Selection" => "Select and move objects (V)",
        "Line" => "Draw a line; Shift snaps to 45 degrees (L)",
        "Rectangle" => "Draw a rectangle; Shift for a square (R)",
        "Ellipse" => "Draw an ellipse; Shift for a circle (E)",
        "Polygon" => "Draw a regular polygon (P)",
        "Star" => "Draw a star with adjustable tips (S)",
        "Text" => "Insert editable text (T)",
        "Measure" => "Measure a distance and angle",
        "Bézier" => "Draw a Bezier path node by node",
        "Freehand" => "Draw a freehand path (B)",
        "Pan" => "Pan the canvas",
        "Insert image..." => "Place a raster image in the document",
        "Edit nodes" => "Edit the nodes of one path",
        "Trim segments" => "Trim the selected segments at their intersections",
        "Join paths" => "Join open paths into one path",
        "New operation" => "Create a cut operation",
        "Compare operations..." => "Compare the parameters of several operations",
        "Preview and G-code..." => "Show the job, preview and playback",
        "Estimate duration" => "Estimate the job duration without writing a file",
        "Apply profile" => "Apply a saved operation profile",
        "Duplicate operation" => "Copy the active operation",
        "Assign operation to layer" => "Assign the active operation to the layer objects",
        "Open library..." | "Materials and operations..." => {
            "Open the materials and operations library"
        }
        "Import profiles..." => "Import a profile library from a .json file",
        "Export profiles..." => "Save the profile library to a .json file",
        "Bring to front" => "Move the selection to the front",
        "Send to back" => "Move the selection to the back",
        "Centre on page" => "Centre the selection on the work area",
        "Distribute on page horizontally" => "Spread the selection across the page",
        "Distribute on page vertically" => "Spread the selection down the page",
        "Rotate 90° left" => "Rotate the selection a quarter turn left",
        "Rotate 90° right" => "Rotate the selection a quarter turn right",
        "Flip horizontal" => "Mirror the selection left to right",
        "Flip vertical" => "Mirror the selection top to bottom",
        "Scale up 10 %" => "Grow the selection by ten percent",
        "Scale down 10 %" => "Shrink the selection by ten percent",
        "Text to outlines" => "Convert text into editable path contours",
        "Fit text to path" => "Lay the text along the selected path",
        "Transform around" => "Choose the centre used by transforms",
        "Selection centre" => "Transform around the selection centre",
        "Page centre" => "Transform around the page centre",
        "Each object's centre" => "Transform each object around its own centre",
        "Reset transforms" => "Restore the original transforms",
        "Skew..." => "Shear the selection by an angle",
        "Grid array..." => "Copy the selection in rows and columns",
        "Close paths" => "Close open paths",
        "Exit node editing" => "Leave node editing",
        "Simplify path..." => "Remove nodes within a tolerance",
        "Repeat..." => "Repeat the selection in a line or around a circle",
        "Nest selection..." => "Arrange copies inside the work area",
        "Find duplicates..." => "Report identical objects",
        "Find intersections..." => "Report intersecting objects",
        "Outline stroke" => "Turn strokes into outlines",
        "Delete overlapping segments" => "Remove duplicated path segments",
        "Offset..." => "Expand or contract the selection",
        "Distribute horizontally" => "Space the objects evenly across",
        "Distribute vertically" => "Space the objects evenly down",
        "Align left" => "Align the left edges",
        "Align centre horizontally" => "Align the horizontal centres",
        "Align right" => "Align the right edges",
        "Align top" => "Align the top edges",
        "Align centre vertically" => "Align the vertical centres",
        "Align bottom" => "Align the bottom edges",
        "Show selection" => "Hide or show the selection frame and handles",
        "Show rulers" => "Show the millimetre rulers",
        "Show guides" => "Show the document guides",
        "Show toolpaths" => "Draw the planned toolpaths on the canvas",
        "Show grid" => "Show the drawing grid",
        "Snap to guides" => "Snap drawing to visible guides",
        "Snap to grid" => "Snap drawing to the grid",
        "Guides..." => "Add and remove horizontal and vertical guides",
        "Zoom in" => "Zoom in",
        "Zoom out" => "Zoom out",
        "Fit page" | "Fit" => "Fit the page in the window",
        "Zoom to selection" => "Zoom to the selected objects",
        "Dark mode" => "Switch between dark and light themes",
        "Layers panel (F9)" => "Show or hide the layers panel",
        "Properties panel (F10)" => "Show or hide the properties panel",
        "About BeamWright" => "Version, scope and licence",
        "Cuts / Layers..." => "Edit cuts, speeds, power and output",
        "Exact size" => "Draw with an exact width and height",
        "Grid" => "Snap or show the grid",
        "Snap" => "Enable snapping",
        "Toolpaths" => "Show the planned toolpaths",
        "Open" => "Open a project",
        "Union" => "Merge the two selected objects",
        "Difference" => "Subtract the second object from the first",
        "Intersection" => "Keep only the overlapping area",
        "Xor" => "Keep the areas that do not overlap",
        "Grid array" => "Copy the selection in a grid",
        "Circular array" => "Copy the selection around a circle",
        "Offset" => "Offset the selection by the radius",
        "Adjust image..." => "Adjust the preparation of the selected image",
        "Nodes" => "Snap to path nodes",
        "Endpoints" => "Snap to the ends of open paths",
        "Centres" => "Snap to object centres",
        "Midpoints" => "Snap to segment midpoints",
        "Intersections" => "Snap to outline crossings",
        "Object limits" => "Snap to object bounds",
        "Appearance" => "Theme and fonts",
        _ => {
            if label.starts_with("Undo") {
                "Undo the last action (Ctrl+Z)"
            } else if label.starts_with("Redo") {
                "Redo the last undone action (Ctrl+Y)"
            } else {
                return None;
            }
        }
    };
    Some(text)
}

fn with_tooltip(response: Response, label: &str) -> Response {
    match tooltip(label) {
        Some(text) => response.on_hover_text(text),
        None => response,
    }
}

/// Button with a leading icon and its hover description.
pub struct IconWidget {
    button: egui::Button<'static>,
    label: String,
}

impl egui::Widget for IconWidget {
    fn ui(self, ui: &mut Ui) -> Response {
        let label = self.label;
        let response = ui.add(self.button);
        with_tooltip(response, &label)
    }
}

/// A button with a leading icon, keeping the standard button behaviour.
///
/// The icon follows the button text colour, so it dims with a disabled button
/// and matches the highlight of a selected row. Hovering shows the description
/// from [`tooltip`] when one exists.
pub fn widget(ui: &Ui, icon: Icon, text: impl Into<WidgetText>) -> IconWidget {
    widget_at(ui, icon, text, ICON_SIZE)
}

/// Like [`widget`], with an explicit icon size for the toolbar.
pub fn widget_at(ui: &Ui, icon: Icon, text: impl Into<WidgetText>, size: f32) -> IconWidget {
    let text = text.into();
    let label = text.text().to_owned();
    IconWidget {
        button: egui::Button::new((image_at(ui.ctx(), icon, size), text))
            .gap(6.0)
            .image_tint_follows_text_color(true),
        label,
    }
}

/// Icon and label for a menu title, tinted like the surrounding text.
pub fn menu_label(ui: &Ui, icon: Icon, text: &str) -> (Image<'static>, String) {
    (
        image(ui.ctx(), icon).tint(ui.visuals().text_color()),
        text.to_owned(),
    )
}

/// Add a plain icon button.
pub fn button(ui: &mut Ui, icon: Icon, text: impl Into<WidgetText>) -> Response {
    ui.add(widget(ui, icon, text))
}

/// Add an icon button that may be disabled.
pub fn button_enabled(
    ui: &mut Ui,
    enabled: bool,
    icon: Icon,
    text: impl Into<WidgetText>,
) -> Response {
    ui.add_enabled(enabled, widget(ui, icon, text))
}

/// Add a selectable row, used for the active tool and radio choices.
pub fn selectable(
    ui: &mut Ui,
    selected: bool,
    icon: Icon,
    text: impl Into<WidgetText>,
) -> Response {
    selectable_at(ui, selected, icon, text, ICON_SIZE)
}

/// Like [`selectable`], with an explicit icon size for the toolbar.
pub fn selectable_at(
    ui: &mut Ui,
    selected: bool,
    icon: Icon,
    text: impl Into<WidgetText>,
    size: f32,
) -> Response {
    let text = text.into();
    let label = text.text().to_owned();
    ui.add(IconWidget {
        button: egui::Button::selectable(selected, (image_at(ui.ctx(), icon, size), text))
            .image_tint_follows_text_color(true),
        label,
    })
}

/// Add a selectable row bound to `current`, mirroring [`Ui::selectable_value`].
pub fn selectable_value<T: PartialEq>(
    ui: &mut Ui,
    current: &mut T,
    value: T,
    icon: Icon,
    text: impl Into<WidgetText>,
) -> Response {
    let selected = *current == value;
    let mut response = selectable(ui, selected, icon, text);
    if response.clicked() && *current != value {
        *current = value;
        response.mark_changed();
    }
    response
}

/// Add a checkbox row with a leading icon, mirroring [`Ui::checkbox`].
pub fn checkbox(
    ui: &mut Ui,
    checked: &mut bool,
    icon: Icon,
    text: impl Into<WidgetText>,
) -> Response {
    checkbox_at(ui, checked, icon, text, ICON_SIZE)
}

/// Icon-only button for the vertical tool strip.
pub fn icon_button(ui: &mut Ui, selected: bool, icon: Icon, size: f32, label: &str) -> Response {
    let image = image_at(ui.ctx(), icon, size);
    let button = egui::Button::selectable(selected, image).image_tint_follows_text_color(true);
    let response = ui.add(button);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label)
    });
    response
}

/// Icon-only button that may be disabled.
pub fn icon_button_enabled(
    ui: &mut Ui,
    enabled: bool,
    icon: Icon,
    size: f32,
    label: &str,
) -> Response {
    let image = image_at(ui.ctx(), icon, size);
    let button = egui::Button::new(image).image_tint_follows_text_color(true);
    let response = ui.add_enabled(enabled, button);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    response
}

/// Like [`checkbox`], with an explicit icon size for the toolbar.
pub fn checkbox_at(
    ui: &mut Ui,
    checked: &mut bool,
    icon: Icon,
    text: impl Into<WidgetText>,
    size: f32,
) -> Response {
    let text = text.into();
    let label = text.text().to_owned();
    let image = image_at(ui.ctx(), icon, size).tint(ui.visuals().text_color());
    with_tooltip(ui.checkbox(checked, (image, text)), &label)
}

#[cfg(test)]
mod tests {
    use super::{GRID, Icon, TEXTURE_SIZE, prims};
    use egui::ColorImage;

    fn coverage(image: &ColorImage) -> usize {
        image.pixels.iter().filter(|pixel| pixel.a() > 0).count()
    }

    #[test]
    fn every_icon_draws_something() {
        for icon in Icon::ALL {
            let image = icon.raster();
            assert_eq!(image.size, [TEXTURE_SIZE, TEXTURE_SIZE]);
            let painted = coverage(&image);
            assert!(
                painted > 20,
                "{icon:?} painted only {painted} pixels; the drawing is empty"
            );
        }
    }

    #[test]
    fn icons_stay_inside_the_grid() {
        for icon in Icon::ALL {
            let image = icon.raster();
            let size = image.size[0];
            let mut edge = 0;
            for i in 0..size {
                for (x, y) in [(i, 0), (i, size - 1), (0, i), (size - 1, i)] {
                    if image.pixels[y * size + x].a() > 0 {
                        edge += 1;
                    }
                }
            }
            assert_eq!(edge, 0, "{icon:?} touches the texture edge");
        }
    }

    /// Icons that deliberately share a drawing with a close relative.
    fn known_alias(a: Icon, b: Icon) -> bool {
        matches!(
            (a, b),
            (Icon::File, Icon::Open)
                | (Icon::Open, Icon::File)
                | (Icon::Draw, Icon::Freehand)
                | (Icon::Freehand, Icon::Draw)
                | (Icon::Edit, Icon::Pencil)
                | (Icon::Pencil, Icon::Edit)
                | (Icon::Operations, Icon::Gear)
                | (Icon::Gear, Icon::Operations)
                | (Icon::Materials, Icon::Palette)
                | (Icon::Palette, Icon::Materials)
                | (Icon::Arrange, Icon::Layers)
                | (Icon::Layers, Icon::Arrange)
                | (Icon::Align, Icon::AlignLeft)
                | (Icon::AlignLeft, Icon::Align)
                | (Icon::Profiles, Icon::Book)
                | (Icon::Book, Icon::Profiles)
                | (Icon::View, Icon::Eye)
                | (Icon::Eye, Icon::View)
                | (Icon::Measure, Icon::Rulers)
                | (Icon::Rulers, Icon::Measure)
        )
    }

    #[test]
    fn icons_are_distinct() {
        let images: Vec<_> = Icon::ALL.iter().map(|icon| icon.raster()).collect();
        for (i, a) in images.iter().enumerate() {
            for (j, b) in images.iter().enumerate().skip(i + 1) {
                if Icon::ALL[i] == Icon::ALL[j] || known_alias(Icon::ALL[i], Icon::ALL[j]) {
                    continue;
                }
                let same = a
                    .pixels
                    .iter()
                    .zip(&b.pixels)
                    .filter(|(left, right)| left.a() != right.a())
                    .count();
                assert!(
                    same > 4,
                    "{:?} and {:?} look identical",
                    Icon::ALL[i],
                    Icon::ALL[j]
                );
            }
        }
    }

    #[test]
    fn prims_are_never_empty() {
        for icon in Icon::ALL {
            assert!(!prims(*icon).is_empty(), "{icon:?} has no primitives");
        }
    }

    #[test]
    fn grid_and_texture_sizes_are_consistent() {
        assert_eq!(GRID, 16.0);
        assert_eq!(TEXTURE_SIZE, 32);
    }
}
