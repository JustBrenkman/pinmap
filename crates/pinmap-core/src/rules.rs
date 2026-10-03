use std::collections::HashMap;

use indexmap::IndexSet;
use pinmap_model::PinId;

use crate::{Diagnostic, Origin, Resolved};

/// Runs every assignment check on a merged project.
pub(crate) fn check(r: &Resolved, diagnostics: &mut Vec<Diagnostic>) {
    let package_known = check_package(r, diagnostics);
    let known: IndexSet<&str> = r.device_peripherals().into_iter().collect();

    for (name, periph) in &r.peripherals {
        if !known.contains(name.as_str()) {
            let message = format!("{name} has no functions in {}", r.pindef.device.name);
            let diag = Diagnostic::error("PM0106", message).with_hint("pinmap periph list");
            diagnostics.push(located(diag, &periph.origin));
            continue;
        }
        for (signal, s) in &periph.signals {
            let pin = &s.asg.pin;
            let Some(def) = r.pindef.pins.get(pin) else {
                diagnostics.push(unknown_pin(r, pin, &s.origin));
                continue;
            };
            if s.mux.is_none() {
                let functions: Vec<String> = def
                    .functions
                    .iter()
                    .map(|f| format!("{}_{}", f.periph, f.name))
                    .collect();
                let functions = if functions.is_empty() {
                    "none".to_owned()
                } else {
                    functions.join(", ")
                };
                let message = format!(
                    "{name}.{signal}: {pin} has no function {name}_{signal} (functions: {functions})"
                );
                let diag = Diagnostic::error("PM0102", message)
                    .with_hint(format!("pinmap pin list --can {name}.{signal}"));
                diagnostics.push(located(diag, &s.origin));
            }
            if package_known {
                check_pad(r, pin, &s.origin, diagnostics);
            }
        }
    }

    for (pin, p) in &r.pins {
        if !r.pindef.pins.contains_key(pin) {
            diagnostics.push(unknown_pin(r, pin, &p.origin));
            continue;
        }
        if package_known {
            check_pad(r, pin, &p.origin, diagnostics);
        }
    }

    check_shared_pins(r, diagnostics);
    check_shared_nets(r, diagnostics);
}

fn located(diagnostic: Diagnostic, origin: &Origin) -> Diagnostic {
    diagnostic.in_file(&origin.file).at_path(&origin.path)
}

fn unknown_pin(r: &Resolved, pin: &PinId, origin: &Origin) -> Diagnostic {
    let message = format!("{pin} is not a pin of {}", r.pindef.device.name);
    located(
        Diagnostic::error("PM0101", message).with_hint("pinmap pin list"),
        origin,
    )
}

/// Reports an unknown target package. Returns whether the package is known.
fn check_package(r: &Resolved, diagnostics: &mut Vec<Diagnostic>) -> bool {
    if r.pindef.packages.contains_key(&r.target.package) {
        return true;
    }
    let known: Vec<&str> = r.pindef.packages.keys().map(String::as_str).collect();
    let known = if known.is_empty() {
        "none".to_owned()
    } else {
        known.join(", ")
    };
    let message = format!(
        "package {} is not in the pindef (packages: {known})",
        r.target.package
    );
    diagnostics.push(
        Diagnostic::error("PM0105", message)
            .in_file(&r.root)
            .at_path("target.package"),
    );
    false
}

fn check_pad(r: &Resolved, pin: &PinId, origin: &Origin, diagnostics: &mut Vec<Diagnostic>) {
    if r.pad(pin.as_str()).is_none() {
        let message = format!("{pin} has no pad in package {}", r.target.package);
        diagnostics.push(located(Diagnostic::error("PM0104", message), origin));
    }
}

/// A pin used more than once is reported at every use after the first.
fn check_shared_pins(r: &Resolved, diagnostics: &mut Vec<Diagnostic>) {
    for (pin, uses) in &r.usage {
        let Some((first, later)) = uses.split_first() else {
            continue;
        };
        for usage in later {
            let message = format!("{pin} is already used by {first}");
            diagnostics.push(located(
                Diagnostic::error("PM0103", message),
                r.origin_of(pin, usage),
            ));
        }
    }
}

fn check_shared_nets(r: &Resolved, diagnostics: &mut Vec<Diagnostic>) {
    let mut first_pin: HashMap<&str, &PinId> = HashMap::new();
    for (pin, uses) in &r.usage {
        let Some(net) = r.net(pin.as_str()) else {
            continue;
        };
        match first_pin.get(net).copied() {
            None => {
                first_pin.insert(net, pin);
            }
            Some(other) => {
                let message = format!("net {net} is on both {other} and {pin}");
                let origin = r.origin_of(pin, &uses[0]);
                diagnostics.push(located(Diagnostic::info("PM0110", message), origin));
            }
        }
    }
}
