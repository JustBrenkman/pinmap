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

/// What a [`Usage`] refers to: a peripheral signal or a pin entry.
#[derive(Debug, Clone, Copy)]
pub enum Assignment<'a> {
    Signal(&'a ResolvedSignal),
    Pin(&'a ResolvedPin),
}

impl<'a> Assignment<'a> {
    pub fn net(self) -> Option<&'a str> {
        match self {
            Self::Signal(s) => s.asg.net.as_deref(),
            Self::Pin(p) => p.asg.net.as_deref(),
        }
    }

    pub fn origin(self) -> &'a Origin {
        match self {
            Self::Signal(s) => &s.origin,
            Self::Pin(p) => &p.origin,
        }
    }

    /// The mux selecting a signal's function; pin entries have none.
    pub fn mux(self) -> Option<&'a Mux> {
        match self {
            Self::Signal(s) => s.mux.as_ref(),
            Self::Pin(_) => None,
        }
    }

    /// A pin entry's note. Signals have none: notes belong to the peripheral.
    pub fn note(self) -> Option<&'a str> {
        match self {
            Self::Signal(_) => None,
            Self::Pin(p) => p.asg.note.as_deref(),
        }
    }

    /// The electrical settings as `key=value` strings, in a fixed order:
    /// bias, drive, speed for signals; initial, bias for pin entries.
    pub fn settings(self) -> Vec<String> {
        let pairs = match self {
            Self::Signal(s) => vec![
                ("bias", s.asg.bias.map(Bias::as_str)),
                ("drive", s.asg.drive.map(Drive::as_str)),
                ("speed", s.asg.speed.map(Speed::as_str)),
            ],
            Self::Pin(p) => vec![
                ("initial", p.asg.initial.map(Level::as_str)),
                ("bias", p.asg.bias.map(Bias::as_str)),
            ],
        };
        pairs
            .into_iter()
            .filter_map(|(key, value)| value.map(|v| format!("{key}={v}")))
            .collect()
    }
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
        self.uses(pin)
            .iter()
            .find_map(|usage| self.assignment(pin, usage).net())
    }

    /// The signal or pin entry behind one of `pin`'s [`Resolved::uses`].
    pub fn assignment(&self, pin: &str, usage: &Usage) -> Assignment<'_> {
        match usage {
            Usage::Signal { periph, signal } => {
                Assignment::Signal(&self.peripherals[periph].signals[signal])
            }
            Usage::Pin { .. } => Assignment::Pin(&self.pins[pin]),
        }
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
