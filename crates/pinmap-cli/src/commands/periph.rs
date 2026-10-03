use std::fmt::Write as _;

use anyhow::bail;
use indexmap::{IndexMap, IndexSet};
use pinmap_core::{Assignment, Origin, Resolved, suggest};
use pinmap_model::{Mux, PinId};
use serde::Serialize;

use crate::output::{Ctx, Exit, Render, dash, joined_or_dash, table};
use crate::project;

#[derive(Serialize)]
pub struct PeriphRow {
    pub periph: String,
    pub assigned: bool,
    pub signals: usize,
}

#[derive(Serialize)]
pub struct PeriphList {
    pub peripherals: Vec<PeriphRow>,
}

impl Render for PeriphList {
    fn render(&self, out: &mut String) {
        let rows: Vec<Vec<String>> = self
            .peripherals
            .iter()
            .map(|p| {
                let assigned = if p.assigned { "yes" } else { "" };
                let signals = if p.assigned {
                    p.signals.to_string()
                } else {
                    "-".into()
                };
                vec![p.periph.clone(), assigned.into(), signals]
            })
            .collect();
        table(out, &["PERIPH", "ASSIGNED", "SIGNALS"], &rows);
    }
}

/// Every peripheral the device has, then any assigned ones it lacks.
fn all_peripherals(r: &Resolved) -> IndexSet<&str> {
    let mut names: IndexSet<&str> = r.device_peripherals().into_iter().collect();
    names.extend(r.peripherals.keys().map(String::as_str));
    names
}

pub fn list(ctx: &Ctx) -> anyhow::Result<Exit> {
    let r = project::load_resolved(ctx)?;
    let peripherals = all_peripherals(&r)
        .into_iter()
        .map(|name| {
            let assigned = r.peripherals.get(name);
            PeriphRow {
                periph: name.to_owned(),
                assigned: assigned.is_some(),
                signals: assigned.map_or(0, |p| p.signals.len()),
            }
        })
        .collect();
    ctx.emit(&PeriphList { peripherals })?;
    Ok(Exit::Ok)
}

#[derive(Serialize)]
pub struct SignalRow {
    pub signal: String,
    pub pin: Option<PinId>,
    pub net: Option<String>,
    pub mux: Option<Mux>,
    pub candidates: Vec<PinId>,
    pub config: Vec<String>,
}

#[derive(Serialize)]
pub struct PeriphShow {
    pub periph: String,
    pub assigned: bool,
    pub signals: Vec<SignalRow>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub props: IndexMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
}

impl Render for PeriphShow {
    fn render(&self, out: &mut String) {
        match &self.origin {
            Some(origin) => {
                let file = origin.file.display();
                let _ = writeln!(out, "{}  assigned in {file}  {}", self.periph, origin.path);
            }
            None => {
                let _ = writeln!(out, "{}  not assigned", self.periph);
            }
        }
        if let Some(note) = &self.note {
            let _ = writeln!(out, "note  {note}");
        }
        out.push('\n');
        let rows: Vec<Vec<String>> = self
            .signals
            .iter()
            .map(|s| {
                vec![
                    s.signal.clone(),
                    dash(s.pin.as_ref().map(PinId::as_str)),
                    dash(s.net.as_deref()),
                    s.mux.as_ref().map_or("-".into(), Mux::to_string),
                    joined_or_dash(&s.candidates, " "),
                    joined_or_dash(&s.config, " "),
                ]
            })
            .collect();
        table(
            out,
            &["SIGNAL", "PIN", "NET", "MUX", "CANDIDATES", "CONFIG"],
            &rows,
        );
        if !self.props.is_empty() {
            out.push_str("\nprops\n");
            for (key, value) in &self.props {
                let _ = writeln!(out, "  {key} = {value}");
            }
        }
    }
}

pub fn show(ctx: &Ctx, name: &str) -> anyhow::Result<Exit> {
    let r = project::load_resolved(ctx)?;
    let known = all_peripherals(&r);
    if !known.contains(name) {
        match suggest(name, known.iter().copied()) {
            Some(p) => bail!("unknown peripheral {name}; did you mean {p}?"),
            None => bail!("unknown peripheral {name}; run `pinmap periph list` to see them all"),
        }
    }
    let assigned = r.peripherals.get(name);

    // Assigned signals first, in file order, then the rest in pindef order.
    let mut signals: IndexSet<&str> = assigned
        .map(|p| p.signals.keys().map(String::as_str).collect())
        .unwrap_or_default();
    for pin in r.pindef.pins.values() {
        for function in pin.functions.iter().filter(|f| f.periph == name) {
            signals.insert(&function.name);
        }
    }

    let rows = signals
        .into_iter()
        .map(|signal| {
            let resolved = assigned.and_then(|p| p.signals.get(signal));
            SignalRow {
                signal: signal.to_owned(),
                pin: resolved.map(|s| s.asg.pin.clone()),
                net: resolved.and_then(|s| s.asg.net.clone()),
                mux: resolved.and_then(|s| s.mux.clone()),
                candidates: r
                    .pins_with_function(name, signal)
                    .map(|(id, _)| id.clone())
                    .collect(),
                config: resolved
                    .map(|s| Assignment::Signal(s).settings())
                    .unwrap_or_default(),
            }
        })
        .collect();
    let report = PeriphShow {
        periph: name.to_owned(),
        assigned: assigned.is_some(),
        signals: rows,
        props: assigned.map(|p| p.props.clone()).unwrap_or_default(),
        note: assigned.and_then(|p| p.note.clone()),
        origin: assigned.map(|p| p.origin.clone()),
    };
    ctx.emit(&report)?;
    Ok(Exit::Ok)
}
