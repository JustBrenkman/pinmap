//! The pin rows that the emitters share.

use pinmap_core::{OptionSpec, Resolved, Usage};
use pinmap_model::{Mux, PinId};

pub(crate) const FREE_PINS: OptionSpec = OptionSpec {
    name: "free-pins",
    help: "Also list pins with a pad that nothing uses",
    default: "false",
};

/// One use of a pin, or an unused pin.
pub(crate) struct Row<'a> {
    pub pin: &'a PinId,
    pub pad: Option<&'a str>,
    /// `SPI1.SCK` or `gpio_out`; `None` for an unused pin.
    pub function: Option<String>,
    pub mux: Option<&'a Mux>,
    pub net: Option<&'a str>,
    pub settings: Vec<String>,
    /// The pin entry's note. Peripheral notes belong to the peripheral.
    pub note: Option<&'a str>,
}

/// One row per use of each pin, in pindef order. With `free_pins`, also one
/// row for each unused pin that has a pad.
pub(crate) fn rows(r: &Resolved, free_pins: bool) -> Vec<Row<'_>> {
    let mut rows = Vec::new();
    for pin in r.pindef.pins.keys() {
        let pad = r.pad(pin.as_str());
        let uses = r.uses(pin.as_str());
        if uses.is_empty() && free_pins && pad.is_some() {
            rows.push(Row {
                pin,
                pad,
                function: None,
                mux: None,
                net: None,
                settings: Vec::new(),
                note: None,
            });
        }
        for usage in uses {
            let (mux, net, note) = match usage {
                Usage::Signal { periph, signal } => {
                    let s = &r.peripherals[periph].signals[signal];
                    (s.mux.as_ref(), s.asg.net.as_deref(), None)
                }
                Usage::Pin { .. } => {
                    let p = &r.pins[pin];
                    (None, p.asg.net.as_deref(), p.asg.note.as_deref())
                }
            };
            rows.push(Row {
                pin,
                pad,
                function: Some(usage.to_string()),
                mux,
                net,
                settings: r.settings(pin, usage),
                note,
            });
        }
    }
    rows
}
