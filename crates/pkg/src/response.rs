//! What a command hands back: its result, a text rendering for humans, and stderr warnings.

use serde::Serialize;
use serde_json::Value;

use crate::invocation::OutputFormat;

/// CLI representations of a typed result; path-bearing results use lossy display paths in JSON.
pub trait RenderOutput: Serialize {
    fn render_text(&self) -> String;

    /// # Panics
    /// A custom serializer that rejects its own data violates the renderer contract.
    fn render_json(&self) -> Value {
        serde_json::to_value(self).expect("CLI result fields serialize to JSON")
    }
}

/// A successful command's outcome. The result goes to stdout; warnings go to stderr.
#[derive(Debug)]
pub struct Response {
    pub result: Value,
    pub text: String,
    /// Diagnostics for stderr, such as a store that does not exist yet.
    pub warnings: Vec<String>,
}

impl Response {
    /// # Panics
    ///
    /// When a custom renderer violates the serializable-result contract.
    pub fn new<T: RenderOutput>(result: &T, output: OutputFormat) -> Response {
        Response {
            result: if output == OutputFormat::Json { result.render_json() } else { Value::Null },
            text: if output == OutputFormat::Text { result.render_text() } else { String::new() },
            warnings: Vec::new(),
        }
    }
}
