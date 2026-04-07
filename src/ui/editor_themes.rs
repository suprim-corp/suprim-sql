/// Adaptive color themes for the code editor (JSON editing, SQL, etc.).
///
/// These themes follow Gruvbox palette and adapt to dark/light mode.
use egui_code_editor::ColorTheme;

/// Return an adaptive `ColorTheme` that matches the current egui dark/light mode.
pub fn adaptive_code_theme(dark_mode: bool) -> ColorTheme {
    if dark_mode {
        ColorTheme {
            name: "adaptive-dark",
            dark: true,
            bg: "none",
            cursor: "#a89984",
            selection: "#504945",
            comments: "#928374",
            functions: "#b8bb26",
            keywords: "#fb4934",
            literals: "#ebdbb2",
            numerics: "#d3869b",
            punctuation: "#fe8019",
            strs: "#8ec07c",
            types: "#fabd2f",
            special: "#83a598",
        }
    } else {
        ColorTheme {
            name: "adaptive-light",
            dark: false,
            bg: "none",
            cursor: "#7c6f64",
            selection: "#d5c4a1",
            comments: "#7c6f64",
            functions: "#79740e",
            keywords: "#9d0006",
            literals: "#282828",
            numerics: "#8f3f71",
            punctuation: "#af3a03",
            strs: "#427b58",
            types: "#b57614",
            special: "#af3a03",
        }
    }
}
