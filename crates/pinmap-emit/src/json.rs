//! A flat pin table for scripts: one object per row with pin, pad, function,
//! mux, and net.

use pinmap_core::{
    EmitError, EmitOptions, EmitOutput, Emitter, OptionSpec, OutputFile, Resolved, bool_option,
};
use pinmap_model::{Mux, PinId};
use serde::Serialize;

use crate::table::{FREE_PINS, rows};

pub struct Json;

#[derive(Serialize)]
struct JsonRow<'a> {
    pin: &'a PinId,
    pad: Option<&'a str>,
    function: Option<String>,
    mux: Option<&'a Mux>,
    net: Option<&'a str>,
}

impl Emitter for Json {
    fn id(&self) -> &'static str {
        "json"
    }

    fn description(&self) -> &'static str {
        "Flat pin table for scripts: pin, pad, function, mux, net"
    }

    fn options(&self) -> &'static [OptionSpec] {
        &[FREE_PINS]
    }

    fn emit(&self, r: &Resolved, options: &EmitOptions) -> Result<EmitOutput, EmitError> {
        let free_pins = bool_option(self, options, FREE_PINS.name)?;
        let table: Vec<JsonRow> = rows(r, free_pins)
            .into_iter()
            .map(|row| JsonRow {
                pin: row.pin,
                pad: row.pad,
                function: row.function,
                mux: row.mux,
                net: row.net,
            })
            .collect();
        let mut contents = serde_json::to_string_pretty(&table).expect("pin rows always serialize");
        contents.push('\n');
        Ok(EmitOutput {
            files: vec![OutputFile {
                path: "pinout.json".into(),
                contents,
            }],
            unsupported: Vec::new(),
        })
    }
}
