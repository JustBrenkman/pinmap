use std::fmt;
use std::path::{Path, PathBuf};

use indexmap::{IndexMap, IndexSet};
use pinmap_model::{
    Bias, Drive, Function, Level, Mode, Mux, PinAsgEntry, PinDef, PinId, SignalAsg, Speed, Target,
};
use serde::Serialize;

use crate::{Diagnostic, Project, load, rules, sort};

/// Where an entry was written: a file and a logical path inside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Origin {
    pub file: PathBuf,
    pub path: String,
}

/// One thing that occupies a pin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Usage {
    Signal { periph: String, signal: String },
    Pin { mode: Mode },
}

impl fmt::Display for Usage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Signal { periph, signal } => write!(f, "{periph}.{signal}"),
            Self::Pin { mode } => f.write_str(mode.as_str()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedSignal {
    pub asg: SignalAsg,
    /// The matching function's mux from the pindef; `None` if there is none.
    pub mux: Option<Mux>,
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedPeriph {
    pub signals: IndexMap<String, ResolvedSignal>,
    pub props: IndexMap<String, String>,
    pub note: Option<String>,
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedPin {
    pub asg: PinAsgEntry,
    pub origin: Origin,
}

/// A project merged across its files, with lookups in both directions.
#[derive(Debug, Clone, Serialize)]
pub struct Resolved {
    pub root: PathBuf,
    pub pindef: PinDef,
    pub pindef_path: PathBuf,
    pub target: Target,
    pub peripherals: IndexMap<String, ResolvedPeriph>,
    pub pins: IndexMap<PinId, ResolvedPin>,
    /// Pin to everything that uses it, peripherals first, in file order.
    pub usage: IndexMap<PinId, Vec<Usage>>,
}

impl Resolved {
    /// The pin's pad in the target package.
    pub fn pad(&self, pin: &str) -> Option<&str> {
        let pads = self.pindef.packages.get(&self.target.package)?;
        pads.get(pin).map(String::as_str)
    }

    pub fn uses(&self, pin: &str) -> &[Usage] {
        self.usage.get(pin).map_or(&[], Vec::as_slice)
    }

    /// The net label on the pin, from the first use that has one.
    pub fn net(&self, pin: &str) -> Option<&str> {
        self.uses(pin).iter().find_map(|usage| match usage {
            Usage::Signal { periph, signal } => {
                self.peripherals[periph].signals[signal].asg.net.as_deref()
            }
            Usage::Pin { .. } => self.pins[pin].asg.net.as_deref(),
        })
    }

    pub fn origin_of(&self, pin: &PinId, usage: &Usage) -> &Origin {
        match usage {
            Usage::Signal { periph, signal } => &self.peripherals[periph].signals[signal].origin,
            Usage::Pin { .. } => &self.pins[pin].origin,
        }
    }

    /// The electrical settings of one use as `key=value` strings, in a fixed
    /// order: bias, drive, speed for signals; initial, bias for pin entries.
    pub fn settings(&self, pin: &PinId, usage: &Usage) -> Vec<String> {
        let pairs: Vec<(&str, Option<&str>)> = match usage {
            Usage::Signal { periph, signal } => {
                let a = &self.peripherals[periph].signals[signal].asg;
                vec![
                    ("bias", a.bias.map(Bias::as_str)),
                    ("drive", a.drive.map(Drive::as_str)),
                    ("speed", a.speed.map(Speed::as_str)),
                ]
            }
            Usage::Pin { .. } => {
                let a = &self.pins[pin].asg;
                vec![
                    ("initial", a.initial.map(Level::as_str)),
                    ("bias", a.bias.map(Bias::as_str)),
                ]
            }
        };
        pairs
            .into_iter()
            .filter_map(|(key, value)| value.map(|v| format!("{key}={v}")))
            .collect()
    }

    /// Peripheral names that appear in the pindef, in first-seen order.
    pub fn device_peripherals(&self) -> Vec<&str> {
        let mut names = IndexSet::new();
        for pin in self.pindef.pins.values() {
            for function in &pin.functions {
                names.insert(function.periph.as_str());
            }
        }
        names.into_iter().collect()
    }

    /// Pins that can carry `periph.signal`, with the matching function.
    pub fn pins_with_function<'a>(
        &'a self,
        periph: &'a str,
        signal: &'a str,
    ) -> impl Iterator<Item = (&'a PinId, &'a Function)> + 'a {
        self.pindef
            .pins
            .iter()
            .filter_map(move |(id, pin)| Some((id, pin.function(periph, signal)?)))
    }
}

/// Merges a loaded project and runs the checks. `None` when the project has
/// no pindef to resolve against.
pub fn resolve(project: &Project) -> (Option<Resolved>, Vec<Diagnostic>) {
    let (Some(device), Some(root)) = (&project.device, project.files.first()) else {
        return (None, Vec::new());
    };
    let mut resolved = Resolved {
        root: root.path.clone(),
        pindef: device.doc.clone(),
        pindef_path: device.path.clone(),
        target: root.doc.target.clone(),
        peripherals: IndexMap::new(),
        pins: IndexMap::new(),
        usage: IndexMap::new(),
    };
    let mut diagnostics = Vec::new();
    merge(project, &mut resolved, &mut diagnostics);
    index(&mut resolved);
    rules::check(&resolved, &mut diagnostics);
    sort(&mut diagnostics);
    (Some(resolved), diagnostics)
}

/// Loads, merges, and checks the project rooted at `root`.
pub fn open(root: &Path) -> (Option<Resolved>, Vec<Diagnostic>) {
    let (project, mut diagnostics) = load(root);
    let (resolved, more) = resolve(&project);
    diagnostics.extend(more);
    sort(&mut diagnostics);
    (resolved, diagnostics)
}

fn merge(project: &Project, r: &mut Resolved, diagnostics: &mut Vec<Diagnostic>) {
    for file in &project.files {
        for (name, periph) in &file.doc.peripherals {
            let origin = Origin {
                file: file.path.clone(),
                path: format!("peripherals.{name}"),
            };
            if let Some(first) = r.peripherals.get(name) {
                diagnostics.push(duplicate(
                    &format!("peripheral {name}"),
                    &first.origin,
                    &origin,
                ));
                continue;
            }
            let signals = periph
                .signals
                .iter()
                .map(|(signal, asg)| {
                    let signal_origin = Origin {
                        file: file.path.clone(),
                        path: format!("{}.signals.{signal}", origin.path),
                    };
                    let resolved = ResolvedSignal {
                        asg: asg.clone(),
                        mux: None,
                        origin: signal_origin,
                    };
                    (signal.clone(), resolved)
                })
                .collect();
            r.peripherals.insert(
                name.clone(),
                ResolvedPeriph {
                    signals,
                    props: periph.props.clone(),
                    note: periph.note.clone(),
                    origin,
                },
            );
        }
        for (pin, entry) in &file.doc.pins {
            let origin = Origin {
                file: file.path.clone(),
                path: format!("pins.{pin}"),
            };
            if let Some(first) = r.pins.get(pin) {
                diagnostics.push(duplicate(&format!("pin {pin}"), &first.origin, &origin));
                continue;
            }
            r.pins.insert(
                pin.clone(),
                ResolvedPin {
                    asg: entry.clone(),
                    origin,
                },
            );
        }
    }
}

fn duplicate(what: &str, first: &Origin, again: &Origin) -> Diagnostic {
    let message = format!("{what} is already defined in {}", first.file.display());
    Diagnostic::error("PM0108", message)
        .in_file(&again.file)
        .at_path(&again.path)
}

fn index(r: &mut Resolved) {
    for (periph, resolved) in &mut r.peripherals {
        for (signal, s) in &mut resolved.signals {
            s.mux = r
                .pindef
                .pins
                .get(&s.asg.pin)
                .and_then(|pin| pin.function(periph, signal))
                .map(|f| f.mux.clone());
            let usage = Usage::Signal {
                periph: periph.clone(),
                signal: signal.clone(),
            };
            r.usage.entry(s.asg.pin.clone()).or_default().push(usage);
        }
    }
    for (pin, p) in &r.pins {
        r.usage
            .entry(pin.clone())
            .or_default()
            .push(Usage::Pin { mode: p.asg.mode });
    }
}
