use pinmap_model::json_schema;

use crate::args::KindArg;
use crate::output::Exit;

/// Prints the schema as JSON, whether or not `--json` was given.
pub fn run(kind: KindArg) -> anyhow::Result<Exit> {
    let schema = json_schema(kind.into());
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(Exit::Ok)
}
