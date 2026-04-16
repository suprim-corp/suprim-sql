//! Unified icon system — wraps `iconflow` for consistent icon access.
//!
//! Usage: `use crate::ui::icons;` then `icons::ph::PLAY`, `icons::db::TABLE`, `icons::engine::postgresql(16.0)`.
//!
//! Three namespaces:
//! - `ph` — Phosphor icons (UI chung, drop-in replacement cho egui_phosphor)
//! - `db` — Tabler icons cho database objects (tô màu khi render)
//! - `engine` — Devicon brand logos (trả RichText có sẵn brand color)

use std::sync::Arc;

use eframe::egui;
use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, RichText};
use iconflow::{try_icon, Pack, Size, Style};

/// Default icon size for sidebar tree items.
pub const SIDEBAR_ICON: f32 = 16.0;
/// Default text size for sidebar tree items.
pub const SIDEBAR_TEXT: f32 = 14.0;
/// Default icon size for sidebar header (connections count, etc.).
pub const SIDEBAR_HEADER: f32 = 14.0;

// ── Font registration ───────────────────────────────────────────────────────

/// Register all enabled iconflow font packs with egui.
/// MUST be called AFTER egui_phosphor::add_to_fonts — merges into existing fonts.
pub fn install_fonts(fonts: &mut FontDefinitions) {
    for font in iconflow::fonts() {
        fonts.font_data.insert(
            font.family.to_string(),
            Arc::new(FontData::from_static(font.bytes)),
        );
        let family = fonts
            .families
            .entry(FontFamily::Name(font.family.into()))
            .or_default();
        if !family.contains(&font.family.to_string()) {
            family.insert(0, font.family.to_string());
        }
    }
}

// ── Helper ──────────────────────────────────────────────────────────────────

/// Resolve an icon to a `&str` glyph string (single char).
/// Panics if icon name not found — only use with known-good names.
fn glyph(pack: Pack, name: &str) -> (String, &'static str) {
    let icon = try_icon(pack, name, Style::Regular, Size::Regular)
        .unwrap_or_else(|_| panic!("icon not found: {name} in {pack:?}"));
    let ch = char::from_u32(icon.codepoint).unwrap_or('?');
    (ch.to_string(), icon.family)
}

/// Create a RichText for an icon with specific font family, size, and color.
fn icon_text(pack: Pack, name: &str, size: f32, color: Color32) -> RichText {
    let (g, family) = glyph(pack, name);
    RichText::new(g)
        .font(FontId::new(size, FontFamily::Name(family.into())))
        .color(color)
}

/// Create a plain icon string (for format! macros) — caller must set font family.
fn icon_str(pack: Pack, name: &str) -> String {
    let (g, _) = glyph(pack, name);
    g
}

// ── Phosphor icons (UI chung) ───────────────────────────────────────────────
//
// Drop-in replacement cho `egui_phosphor::regular::XXX`.
// Trả `&str`-like String dùng trong format!() — cần set FontFamily khi render.
// Hoặc dùng `ph::rich("name", size)` để lấy RichText có sẵn font.

pub mod ph {
    use super::*;

    /// Render a Phosphor icon as RichText with proper font family.
    pub fn rich(name: &str, size: f32) -> RichText {
        let (g, family) = glyph(Pack::Phosphor, name);
        RichText::new(g).font(FontId::new(size, FontFamily::Name(family.into())))
    }

    /// Render a Phosphor icon as RichText with color.
    pub fn colored(name: &str, size: f32, color: Color32) -> RichText {
        rich(name, size).color(color)
    }

    // String constants for common Phosphor icons used in format!() macros.
    // These need the Phosphor font family to render — use with FontFamily::Name.

    macro_rules! ph_icons {
        ($($const_name:ident => $icon_name:literal),* $(,)?) => {
            $(
                pub fn $const_name() -> String { icon_str(Pack::Phosphor, $icon_name) }
            )*
        };
    }

    ph_icons! {
        // Navigation & actions
        x => "x",
        play => "play",
        pause => "pause",
        plus => "plus",
        minus => "minus",
        trash => "trash",
        pencil => "pencil",
        pencil_simple => "pencil-simple",
        info => "info",
        check_circle => "check-circle",
        x_circle => "x-circle",
        plus_circle => "plus-circle",
        arrow_right => "arrow-right",
        arrow_left => "arrow-left",
        arrow_fat_up => "arrow-fat-up",
        arrow_fat_down => "arrow-fat-down",
        arrow_counter_clockwise => "arrow-counter-clockwise",
        arrow_u_up_left => "arrow-u-up-left",
        arrow_square_up_right => "arrow-square-up-right",
        arrows_clockwise => "arrows-clockwise",
        arrows_left_right => "arrows-left-right",
        arrow_clockwise => "arrows-clockwise",
        caret_right => "caret-right",
        caret_left => "caret-left",

        // UI elements
        bell => "bell",
        crown => "crown",
        list => "list",
        lock_simple => "lock-simple",
        gauge => "gauge",
        sign_in => "sign-in",
        sign_out => "sign-out",
        command => "command",
        key_return => "key-return",
        magic_wand => "magic-wand",
        clipboard_text => "clipboard-text",
        terminal_window => "terminal-window",
        clock => "clock",
        clock_clockwise => "clock-clockwise",
        clock_counter_clockwise => "clock-counter-clockwise",
        hourglass => "hourglass",
        timer => "timer",
        rows => "rows",
        users => "users",
        lightning => "lightning",
        key => "key",

        // Database (generic, from Phosphor)
        database => "database",
        hard_drives => "hard-drives",
        tree_structure => "tree-structure",
        table => "table",
        columns => "columns",
        link => "link",
        eye => "eye",
        squares_four => "squares-four",
        list_numbers => "list-numbers",
        plugs_connected => "plugs-connected",
        magnifying_glass => "magnifying-glass",
        hash => "hash",
        function => "function",
        puzzle_piece => "puzzle-piece",
    }
}

// ── DB Object icons (Tabler — more distinctive for schema browser) ──────────

pub mod db {
    use super::*;

    pub fn database(size: f32, color: Color32) -> RichText {
        icon_text(Pack::Tabler, "database", size, color)
    }
    pub fn table(size: f32, color: Color32) -> RichText {
        icon_text(Pack::Tabler, "table", size, color)
    }
    pub fn column(size: f32, color: Color32) -> RichText {
        icon_text(Pack::Tabler, "columns", size, color)
    }
    pub fn key(size: f32, color: Color32) -> RichText {
        icon_text(Pack::Tabler, "key", size, color)
    }
    pub fn index(size: f32, color: Color32) -> RichText {
        icon_text(Pack::Tabler, "search", size, color)
    }
    pub fn foreign_key(size: f32, color: Color32) -> RichText {
        icon_text(Pack::Tabler, "link", size, color)
    }
    pub fn view(size: f32, color: Color32) -> RichText {
        icon_text(Pack::Tabler, "eye", size, color)
    }
    pub fn sequence(size: f32, color: Color32) -> RichText {
        icon_text(Pack::Tabler, "list-numbers", size, color)
    }
    pub fn func(size: f32, color: Color32) -> RichText {
        icon_text(Pack::Tabler, "code", size, color)
    }
    pub fn schema(size: f32, color: Color32) -> RichText {
        icon_text(Pack::Tabler, "folder", size, color)
    }
    pub fn extension(size: f32, color: Color32) -> RichText {
        icon_text(Pack::Tabler, "puzzle", size, color)
    }
    pub fn trigger(size: f32, color: Color32) -> RichText {
        icon_text(Pack::Tabler, "bolt", size, color)
    }

    // Semantic colors for sidebar tree
    pub const COLOR_DATABASE: Color32 = Color32::from_rgb(66, 165, 245); // blue
    pub const COLOR_SCHEMA: Color32 = Color32::from_rgb(255, 183, 77); // amber
    pub const COLOR_TABLE: Color32 = Color32::from_rgb(102, 187, 106); // green
    pub const COLOR_COLUMN: Color32 = Color32::from_rgb(171, 171, 171); // gray
    pub const COLOR_PK: Color32 = Color32::from_rgb(255, 213, 79); // gold
    pub const COLOR_INDEX: Color32 = Color32::from_rgb(77, 182, 172); // teal
    pub const COLOR_FK: Color32 = Color32::from_rgb(255, 167, 38); // orange
    pub const COLOR_VIEW: Color32 = Color32::from_rgb(186, 104, 200); // purple
    pub const COLOR_SEQUENCE: Color32 = Color32::from_rgb(100, 181, 246); // light blue
    pub const COLOR_FUNCTION: Color32 = Color32::from_rgb(149, 117, 205); // deep purple
    pub const COLOR_EXTENSION: Color32 = Color32::from_rgb(174, 213, 129); // light green
}

// ── DB Engine brand logos (Devicon) ─────────────────────────────────────────

pub mod engine {
    use super::*;

    // Brand colors
    const PG_BLUE: Color32 = Color32::from_rgb(100, 160, 210);
    const MYSQL_TEAL: Color32 = Color32::from_rgb(0, 117, 143);
    const SQLITE_BLUE: Color32 = Color32::from_rgb(0, 123, 194);
    const REDIS_RED: Color32 = Color32::from_rgb(220, 56, 45);
    const MONGO_GREEN: Color32 = Color32::from_rgb(77, 179, 61);
    const MSSQL_RED: Color32 = Color32::from_rgb(204, 41, 54);

    pub fn postgresql(size: f32) -> RichText {
        icon_text(Pack::Devicon, "postgresql-plain", size, PG_BLUE)
    }
    pub fn mysql(size: f32) -> RichText {
        icon_text(Pack::Devicon, "mysql-original", size, MYSQL_TEAL)
    }
    pub fn sqlite(size: f32) -> RichText {
        icon_text(Pack::Devicon, "sqlite-plain", size, SQLITE_BLUE)
    }
    pub fn redis(size: f32) -> RichText {
        icon_text(Pack::Devicon, "redis-plain", size, REDIS_RED)
    }
    pub fn mongodb(size: f32) -> RichText {
        icon_text(Pack::Devicon, "mongodb-plain", size, MONGO_GREEN)
    }
    pub fn mssql(size: f32) -> RichText {
        icon_text(Pack::Devicon, "microsoftsqlserver-plain", size, MSSQL_RED)
    }

    /// Get engine icon by DriverType string name.
    pub fn by_name(name: &str, size: f32) -> RichText {
        match name.to_lowercase().as_str() {
            "postgres" | "postgresql" => postgresql(size),
            "mysql" | "mariadb" => mysql(size),
            "sqlite" => sqlite(size),
            "redis" => redis(size),
            "mongodb" | "mongo" => mongodb(size),
            "mssql" | "sqlserver" | "microsoftsqlserver" => mssql(size),
            _ => icon_text(Pack::Phosphor, "database", size, Color32::GRAY),
        }
    }
}
