use crate::{DocKind, PinAsg, PinDef};

/// The JSON Schema for a document kind, for editor autocomplete and validation.
pub fn json_schema(kind: DocKind) -> schemars::Schema {
    match kind {
        DocKind::Pindef => schemars::schema_for!(PinDef),
        DocKind::Pinasg => schemars::schema_for!(PinAsg),
    }
}
