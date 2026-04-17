//! JSON export options and UI rendering.

use eframe::egui;

#[derive(Debug, Clone)]
pub struct JsonOptions {
    pub pretty_print: bool,
    pub include_null_values: bool,
    pub all_as_strings: bool,
    pub gzip: bool,
}

impl Default for JsonOptions {
    fn default() -> Self {
        Self {
            pretty_print: true,
            include_null_values: true,
            all_as_strings: false,
            gzip: false,
        }
    }
}

pub fn render_options_ui(ui: &mut egui::Ui, opts: &mut JsonOptions) {
    ui.checkbox(&mut opts.pretty_print, "Pretty print (indent)");
    ui.checkbox(&mut opts.include_null_values, "Include NULL values");
    ui.checkbox(&mut opts.all_as_strings, "Preserve all values as strings");
    ui.checkbox(&mut opts.gzip, "Compress the file using Gzip");
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Tip: Enable \"all as strings\" for ZIP codes, phone numbers, IDs.")
            .weak()
            .size(11.0),
    );
}
