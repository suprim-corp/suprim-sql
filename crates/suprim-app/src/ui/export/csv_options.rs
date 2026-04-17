//! CSV export options and UI rendering.
//!
//! Options match TablePro: convert NULL to EMPTY, headers, delimiter, quote handling,
//! line break, sanitize formulas.

use eframe::egui;

// ── Options ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delimiter {
    Comma,
    Semicolon,
    Tab,
    Pipe,
}

impl Delimiter {
    pub fn label(&self) -> &'static str {
        match self {
            Delimiter::Comma => ",",
            Delimiter::Semicolon => ";",
            Delimiter::Tab => "\\t",
            Delimiter::Pipe => "|",
        }
    }
    pub fn char(&self) -> char {
        match self {
            Delimiter::Comma => ',',
            Delimiter::Semicolon => ';',
            Delimiter::Tab => '\t',
            Delimiter::Pipe => '|',
        }
    }
    pub fn all() -> &'static [Delimiter] {
        &[
            Delimiter::Comma,
            Delimiter::Semicolon,
            Delimiter::Tab,
            Delimiter::Pipe,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuoteHandling {
    Always,
    AsNeeded,
    Never,
}

impl QuoteHandling {
    pub fn label(&self) -> &'static str {
        match self {
            QuoteHandling::Always => "Always quote",
            QuoteHandling::AsNeeded => "Quote if needed",
            QuoteHandling::Never => "Never quote",
        }
    }
    pub fn all() -> &'static [QuoteHandling] {
        &[
            QuoteHandling::AsNeeded,
            QuoteHandling::Always,
            QuoteHandling::Never,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineBreak {
    Lf,
    CrLf,
    Cr,
}

impl LineBreak {
    pub fn label(&self) -> &'static str {
        match self {
            LineBreak::Lf => "\\n",
            LineBreak::CrLf => "\\r\\n",
            LineBreak::Cr => "\\r",
        }
    }
    pub fn value(&self) -> &'static str {
        match self {
            LineBreak::Lf => "\n",
            LineBreak::CrLf => "\r\n",
            LineBreak::Cr => "\r",
        }
    }
    pub fn all() -> &'static [LineBreak] {
        &[LineBreak::Lf, LineBreak::CrLf, LineBreak::Cr]
    }
}

#[derive(Debug, Clone)]
pub struct CsvOptions {
    pub convert_null_to_empty: bool,
    pub convert_line_break_to_space: bool,
    pub include_field_names: bool,
    pub sanitize_formulas: bool,
    pub delimiter: Delimiter,
    pub quote_handling: QuoteHandling,
    pub line_break: LineBreak,
    pub gzip: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            convert_null_to_empty: true,
            convert_line_break_to_space: false,
            include_field_names: true,
            sanitize_formulas: true,
            delimiter: Delimiter::Comma,
            quote_handling: QuoteHandling::AsNeeded,
            line_break: LineBreak::Lf,
            gzip: false,
        }
    }
}

// ── UI ──────────────────────────────────────────────────────────────────────

pub fn render_options_ui(ui: &mut egui::Ui, opts: &mut CsvOptions) {
    ui.checkbox(&mut opts.convert_null_to_empty, "Convert NULL to EMPTY");
    ui.checkbox(
        &mut opts.convert_line_break_to_space,
        "Convert line break to space",
    );
    ui.checkbox(
        &mut opts.include_field_names,
        "Put field names in the first row",
    );
    ui.checkbox(&mut opts.sanitize_formulas, "Sanitize formula-like values");
    ui.checkbox(&mut opts.gzip, "Compress the file using Gzip");

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);

    egui::Grid::new("csv_dropdowns")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            // Delimiter
            ui.label("Delimiter");
            egui::ComboBox::from_id_salt("csv_delimiter")
                .selected_text(opts.delimiter.label())
                .width(140.0)
                .show_ui(ui, |ui| {
                    for d in Delimiter::all() {
                        if ui
                            .selectable_label(opts.delimiter == *d, d.label())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            opts.delimiter = d.clone();
                        }
                    }
                });
            ui.end_row();

            // Quote
            ui.label("Quote");
            egui::ComboBox::from_id_salt("csv_quote")
                .selected_text(opts.quote_handling.label())
                .width(140.0)
                .show_ui(ui, |ui| {
                    for q in QuoteHandling::all() {
                        if ui
                            .selectable_label(opts.quote_handling == *q, q.label())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            opts.quote_handling = q.clone();
                        }
                    }
                });
            ui.end_row();

            // Line break
            ui.label("Line break");
            egui::ComboBox::from_id_salt("csv_linebreak")
                .selected_text(opts.line_break.label())
                .width(140.0)
                .show_ui(ui, |ui| {
                    for lb in LineBreak::all() {
                        if ui
                            .selectable_label(opts.line_break == *lb, lb.label())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            opts.line_break = lb.clone();
                        }
                    }
                });
            ui.end_row();
        });
}
