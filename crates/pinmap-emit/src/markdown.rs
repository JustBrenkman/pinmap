//! A pinout document: a table of every assigned pin, then a section per
//! peripheral.

use std::fmt::Write as _;

use pinmap_core::{
    EmitError, EmitOptions, EmitOutput, Emitter, OptionSpec, OutputFile, Resolved, bool_option,
};

use crate::table::{FREE_PINS, rows};

pub struct Markdown;

impl Emitter for Markdown {
    fn id(&self) -> &'static str {
        "markdown"
    }

    fn description(&self) -> &'static str {
        "Pinout document: a pin table and a section per peripheral"
    }

    fn options(&self) -> &'static [OptionSpec] {
        &[FREE_PINS]
    }

    fn emit(&self, r: &Resolved, options: &EmitOptions) -> Result<EmitOutput, EmitError> {
        let free_pins = bool_option(self, options, FREE_PINS.name)?;
        let mut out = String::new();
        let refdes = r
            .target
            .refdes
            .as_deref()
            .map(|d| format!(", {d}"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "# {} pinout ({}{refdes})\n",
            r.pindef.device.name, r.target.package
        );

        out.push_str("| Pin | Pad | Function | Mux | Net | Config | Note |\n");
        out.push_str("|---|---|---|---|---|---|---|\n");
        for row in rows(r, free_pins) {
            table_row(
                &mut out,
                &[
                    row.pin.as_str(),
                    row.pad.unwrap_or(""),
                    row.function.as_deref().unwrap_or(""),
                    &row.mux.map(ToString::to_string).unwrap_or_default(),
                    row.net.unwrap_or(""),
                    &row.settings.join(" "),
                    row.note.unwrap_or(""),
                ],
            );
        }

        for (name, periph) in &r.peripherals {
            let _ = writeln!(out, "\n## {name}\n");
            if let Some(note) = &periph.note {
                let _ = writeln!(out, "{note}\n");
            }
            out.push_str("| Signal | Pin | Net |\n|---|---|---|\n");
            for (signal, s) in &periph.signals {
                let net = s.asg.net.as_deref().unwrap_or("");
                table_row(&mut out, &[signal, s.asg.pin.as_str(), net]);
            }
            if !periph.props.is_empty() {
                out.push_str("\nProperties:\n\n");
                for (key, value) in &periph.props {
                    let _ = writeln!(out, "- `{key} = {value}`");
                }
            }
        }

        Ok(EmitOutput {
            files: vec![OutputFile {
                path: "pinout.md".into(),
                contents: out,
            }],
            unsupported: Vec::new(),
        })
    }
}

/// Writes one table row, escaping each cell.
fn table_row(out: &mut String, cells: &[&str]) {
    let cells: Vec<String> = cells.iter().map(|c| cell(c)).collect();
    let _ = writeln!(out, "| {} |", cells.join(" | "));
}

/// Keeps a value inside its table cell: escapes `|` and flattens line breaks.
fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\r', '\n'], " ")
}
