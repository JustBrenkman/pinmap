use std::fmt::Write as _;

use anyhow::{anyhow, bail};
use pinmap_core::{Origin, Resolved, Usage};
use pinmap_model::{Bias, Drive, Level, Mux, Pin, PinId, Port, Speed};
use serde::Serialize;

use crate::args::PinListArgs;
use crate::output::{Ctx, Exit, Render, dash, mux_text, table};
use crate::project;

#[derive(Serialize)]
pub struct PinRow {
    pub pin: PinId,
    pub pad: Option<String>,
    pub uses: Vec<Usage>,
    pub net: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mux: Option<Mux>,
}

#[derive(Serialize)]
pub struct PinList {
    #[serde(skip)]
    with_mux: bool,
    pub pins: Vec<PinRow>,
}

impl Render for PinList {
    fn render(&self, out: &mut String) {
        if self.pins.is_empty() {
            out.push_str("no pins match\n");
            return;
        }
        let mut header = vec!["PIN", "PAD", "USE", "NET"];
        if self.with_mux {
            header.push("MUX");
        }
        let rows: Vec<Vec<String>> = self
            .pins
            .iter()
            .map(|row| {
                let mut cells = vec![
                    row.pin.to_string(),
                    dash(row.pad.as_deref()),
                    uses_text(&row.uses),
                    dash(row.net.as_deref()),
                ];
                if let Some(mux) = &row.mux {
                    cells.push(mux_text(mux));
                }
                cells
            })
            .collect();
        table(out, &header, &rows);
    }
}

fn uses_text(uses: &[Usage]) -> String {
    if uses.is_empty() {
        return "-".into();
    }
    let names: Vec<String> = uses.iter().map(Usage::to_string).collect();
    names.join(", ")
}

/// Splits `SPI1.SCK` into `("SPI1", "SCK")`.
pub fn parse_signal(text: &str) -> anyhow::Result<(&str, &str)> {
    text.split_once('.')
        .filter(|(periph, signal)| !periph.is_empty() && !signal.is_empty())
        .ok_or_else(|| anyhow!("expected PERIPH.SIGNAL, for example SPI1.SCK, got {text}"))
}

pub fn list(ctx: &Ctx, args: &PinListArgs) -> anyhow::Result<Exit> {
    let can = args.can.as_deref().map(parse_signal).transpose()?;
    let r = project::load_resolved(ctx)?;
    let mut pins = Vec::new();
    for (id, pin) in &r.pindef.pins {
        let uses = r.uses(id.as_str());
        let pad = r.pad(id.as_str());
        if args.free && (pad.is_none() || !uses.is_empty()) {
            continue;
        }
        if args.used && uses.is_empty() {
            continue;
        }
        let net = r.net(id.as_str());
        if args.net.as_deref().is_some_and(|want| net != Some(want)) {
            continue;
        }
        let mux = match can {
            Some((periph, signal)) => {
                match pin
                    .functions
                    .iter()
                    .find(|f| f.periph == periph && f.name == signal)
                {
                    Some(function) => Some(function.mux.clone()),
                    None => continue,
                }
            }
            None => None,
        };
        pins.push(PinRow {
            pin: id.clone(),
            pad: pad.map(str::to_owned),
            uses: uses.to_vec(),
            net: net.map(str::to_owned),
            mux,
        });
    }
    ctx.emit(&PinList {
        with_mux: can.is_some(),
        pins,
    })?;
    Ok(Exit::Ok)
}

#[derive(Serialize)]
pub struct UseDetail {
    pub usage: Usage,
    pub net: Option<String>,
    pub config: Vec<String>,
    pub note: Option<String>,
    pub origin: Origin,
}

#[derive(Serialize)]
pub struct FunctionRow {
    pub periph: String,
    pub name: String,
    pub mux: Mux,
    pub assigned: bool,
}

#[derive(Serialize)]
pub struct PinShow {
    pub pin: PinId,
    pub pad: Option<String>,
    pub port: Option<Port>,
    pub uses: Vec<UseDetail>,
    pub functions: Vec<FunctionRow>,
}

impl Render for PinShow {
    fn render(&self, out: &mut String) {
        let port = self.port.as_ref().map(|p| format!("{}{}", p.bank, p.index));
        let _ = writeln!(out, "pin     {}", self.pin);
        let _ = writeln!(out, "pad     {}", dash(self.pad.as_deref()));
        let _ = writeln!(out, "port    {}", dash(port.as_deref()));
        if self.uses.is_empty() {
            let _ = writeln!(out, "use     -");
        }
        for detail in &self.uses {
            let _ = writeln!(out, "use     {}", detail.usage);
            let _ = writeln!(out, "net     {}", dash(detail.net.as_deref()));
            if !detail.config.is_empty() {
                let _ = writeln!(out, "config  {}", detail.config.join(" "));
            }
            if let Some(note) = &detail.note {
                let _ = writeln!(out, "note    {note}");
            }
            let _ = writeln!(
                out,
                "origin  {}  {}",
                detail.origin.file.display(),
                detail.origin.path
            );
        }
        out.push('\n');
        let rows: Vec<Vec<String>> = self
            .functions
            .iter()
            .map(|f| {
                let assigned = if f.assigned { "yes" } else { "" };
                vec![
                    format!("{}.{}", f.periph, f.name),
                    mux_text(&f.mux),
                    assigned.into(),
                ]
            })
            .collect();
        table(out, &["FUNCTION", "MUX", "ASSIGNED"], &rows);
    }
}

pub fn show(ctx: &Ctx, name: &str) -> anyhow::Result<Exit> {
    let r = project::load_resolved(ctx)?;
    let (id, pin) = find_pin(&r, name)?;
    let uses = r.uses(id.as_str());
    let functions = pin
        .functions
        .iter()
        .map(|f| FunctionRow {
            periph: f.periph.clone(),
            name: f.name.clone(),
            mux: f.mux.clone(),
            assigned: uses.iter().any(|u| {
                matches!(u, Usage::Signal { periph, signal } if *periph == f.periph && *signal == f.name)
            }),
        })
        .collect();
    let report = PinShow {
        pin: id.clone(),
        pad: r.pad(id.as_str()).map(str::to_owned),
        port: pin.port.clone(),
        uses: uses.iter().map(|u| use_detail(&r, id, u)).collect(),
        functions,
    };
    ctx.emit(&report)?;
    Ok(Exit::Ok)
}

fn find_pin<'a>(r: &'a Resolved, name: &str) -> anyhow::Result<(&'a PinId, &'a Pin)> {
    if let Some(found) = r.pindef.pins.get_key_value(name) {
        return Ok(found);
    }
    match r
        .pindef
        .pins
        .keys()
        .find(|id| id.as_str().eq_ignore_ascii_case(name))
    {
        Some(id) => bail!("unknown pin {name}; did you mean {id}?"),
        None => bail!("unknown pin {name}; run `pinmap pin list` to see every pin"),
    }
}

fn use_detail(r: &Resolved, pin: &PinId, usage: &Usage) -> UseDetail {
    match usage {
        Usage::Signal { periph, signal } => {
            let s = &r.peripherals[periph].signals[signal];
            let config = config([
                ("bias", s.asg.bias.map(Bias::as_str)),
                ("drive", s.asg.drive.map(Drive::as_str)),
                ("speed", s.asg.speed.map(Speed::as_str)),
            ]);
            UseDetail {
                usage: usage.clone(),
                net: s.asg.net.clone(),
                config,
                note: None,
                origin: s.origin.clone(),
            }
        }
        Usage::Pin { .. } => {
            let p = &r.pins[pin];
            let config = config([
                ("initial", p.asg.initial.map(Level::as_str)),
                ("bias", p.asg.bias.map(Bias::as_str)),
            ]);
            UseDetail {
                usage: usage.clone(),
                net: p.asg.net.clone(),
                config,
                note: p.asg.note.clone(),
                origin: p.origin.clone(),
            }
        }
    }
}

fn config<const N: usize>(items: [(&str, Option<&str>); N]) -> Vec<String> {
    items
        .into_iter()
        .filter_map(|(key, value)| value.map(|v| format!("{key}={v}")))
        .collect()
}
