use json_patch::diff;
use serde_json::Value;

pub fn compute_json_patch(old: &Value, new: &Value) -> Option<Value> {
    let patch = diff(old, new);
    if patch.0.is_empty() {
        None
    } else {
        Some(serde_json::to_value(patch).unwrap_or(Value::Null))
    }
}

pub fn compute_diff_for_revision(
    revision: &crate::models::entry::EntryRevision,
    previous_revision: Option<&crate::models::entry::EntryRevision>,
) -> Option<Value> {
    match previous_revision {
        Some(prev) => compute_json_patch(&prev.data.0, &revision.data.0),
        None => None,
    }
}
