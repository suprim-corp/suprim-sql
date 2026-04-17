//! CSV export plugin — options, UI, writer.
//!
//! Options match TablePro: convert NULL to EMPTY, headers, delimiter, quote handling,
//! line break, sanitize formulas.

use std::io::Write;
use std::path::Path;

use eframe::egui;

use suprim_core::db::values::{DbValue, QueryResult};

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

// ── Writer ──────────────────────────────────────────────────────────────────

/// Export a single `QueryResult` to CSV with the given options.
pub fn export(result: &QueryResult, path: &Path, opts: &CsvOptions) -> std::io::Result<()> {
    let mut f = super::create_writer(path, opts.gzip)?;
    let sep = opts.delimiter.char();
    let lb = opts.line_break.value();

    // Header row
    if opts.include_field_names {
        let headers: Vec<String> = result
            .columns
            .iter()
            .map(|c| format_cell(&c.name, sep, opts))
            .collect();
        write!(f, "{}{}", headers.join(&sep.to_string()), lb)?;
    }

    // Data rows
    for row in &result.rows {
        let cells: Vec<String> = row.iter().map(|v| format_db_value(v, sep, opts)).collect();
        write!(f, "{}{}", cells.join(&sep.to_string()), lb)?;
    }

    Ok(())
}

/// Format a `DbValue` as a CSV cell string.
fn format_db_value(val: &DbValue, sep: char, opts: &CsvOptions) -> String {
    match val {
        DbValue::Null => {
            if opts.convert_null_to_empty {
                String::new()
            } else {
                "NULL".to_string()
            }
        }
        DbValue::Bool(b) => b.to_string(),
        DbValue::Int(i) => i.to_string(),
        DbValue::Float(f) => f.to_string(),
        DbValue::Timestamp(t) => format_cell(&t.format("%Y-%m-%d %H:%M:%S").to_string(), sep, opts),
        DbValue::Bytes(b) => {
            let hex: String = b.iter().map(|byte| format!("{:02x}", byte)).collect();
            format_cell(&format!("\\x{hex}"), sep, opts)
        }
        DbValue::Text(s) => format_cell(s, sep, opts),
        DbValue::Json(v) => format_cell(&v.to_string(), sep, opts),
    }
}

/// Apply quote + escape rules + formula sanitation + line-break conversion to a string cell.
fn format_cell(raw: &str, sep: char, opts: &CsvOptions) -> String {
    let mut s = raw.to_string();

    // Convert line breaks to spaces if requested.
    if opts.convert_line_break_to_space {
        s = s.replace(['\n', '\r'], " ");
    }

    // Sanitize formula-like values (prevent CSV injection).
    if opts.sanitize_formulas {
        if let Some(first) = s.chars().next() {
            if matches!(first, '=' | '+' | '-' | '@' | '\t' | '\r') {
                s = format!("'{s}");
            }
        }
    }

    // Quote handling
    let needs_quote = s.contains(sep) || s.contains('"') || s.contains('\n') || s.contains('\r');
    match opts.quote_handling {
        QuoteHandling::Never => s,
        QuoteHandling::Always => format!("\"{}\"", s.replace('"', "\"\"")),
        QuoteHandling::AsNeeded => {
            if needs_quote {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s
            }
        }
    }
}
