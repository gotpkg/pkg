use std::path::Path;

use pkg_core::ErrorDetails;
use serde::Serialize;
use serde_json::{Map, Value, json};

pub(crate) fn render(details: &ErrorDetails) -> Value {
    let mut fields = Map::new();
    optional(&mut fields, "package", details.package.as_ref());
    optional(&mut fields, "packages", details.packages.as_ref());
    optional(&mut fields, "installed", details.installed.as_ref());
    optional(&mut fields, "config", details.config.as_deref().map(Path::to_string_lossy));
    if let Some(paths) = &details.configs {
        fields.insert(
            "configs".into(),
            Value::Array(paths.iter().map(|path| json!(path.to_string_lossy())).collect()),
        );
    }
    optional(
        &mut fields,
        "editableConfig",
        details.editable_config.as_deref().map(Path::to_string_lossy),
    );
    optional(&mut fields, "undeclared", details.undeclared.as_ref());
    optional(&mut fields, "tap", details.tap.as_ref());
    optional(&mut fields, "url", details.url.as_ref());
    optional(&mut fields, "command", details.command.as_ref());
    optional(&mut fields, "status", details.status);
    optional(&mut fields, "output", details.output.as_ref());
    optional(&mut fields, "program", details.program.as_deref().map(Path::to_string_lossy));
    optional(&mut fields, "variable", details.variable.as_ref());
    optional(&mut fields, "argument", details.argument.as_ref());
    optional(&mut fields, "value", details.value.as_ref());
    optional(&mut fields, "path", details.path.as_deref().map(Path::to_string_lossy));
    optional(&mut fields, "phase", details.phase.as_ref());
    optional(&mut fields, "completed", details.completed.as_ref());
    optional(&mut fields, "changesMayRemain", details.changes_may_remain);
    Value::Object(fields)
}

fn optional<T: Serialize>(fields: &mut Map<String, Value>, key: &str, value: Option<T>) {
    if let Some(value) = value {
        fields.insert(key.into(), json!(value));
    }
}
