//! The error every failing command reports on stderr: text for people, JSON with `--output json`.

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

/// A stable error code. Codes, exit statuses, and retryability are part of the CLI contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// Input is missing or malformed.
    InvalidArgument,
    /// A named package, tap, command, or document does not exist.
    NotFound,
    /// The machine or the config is not in a state the command can work with.
    PreconditionFailed,
    /// A package catalog could not be reached; retrying can succeed.
    Unavailable,
    /// mise ran and reported a failure.
    MiseFailed,
    /// A defect in `pkg` itself, or a stdout that cannot take the result.
    Internal,
}

impl ErrorCode {
    /// Every code, in the order the reference lists them.
    pub const ALL: [ErrorCode; 6] = [
        ErrorCode::InvalidArgument,
        ErrorCode::NotFound,
        ErrorCode::PreconditionFailed,
        ErrorCode::Unavailable,
        ErrorCode::MiseFailed,
        ErrorCode::Internal,
    ];

    /// The code as it appears in the error JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::InvalidArgument => "INVALID_ARGUMENT",
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::PreconditionFailed => "PRECONDITION_FAILED",
            ErrorCode::Unavailable => "UNAVAILABLE",
            ErrorCode::MiseFailed => "MISE_FAILED",
            ErrorCode::Internal => "INTERNAL",
        }
    }

    /// The process exit status the code sets. Success is always 0.
    pub fn exit_status(self) -> u8 {
        match self {
            ErrorCode::InvalidArgument => 2,
            ErrorCode::NotFound | ErrorCode::PreconditionFailed => 3,
            ErrorCode::Unavailable => 5,
            ErrorCode::MiseFailed => 6,
            ErrorCode::Internal => 70,
        }
    }

    /// Whether running the same command again, unchanged, can succeed.
    pub fn retryable(self) -> bool {
        matches!(self, ErrorCode::Unavailable)
    }

    /// One sentence for the reference and `schema`.
    pub fn meaning(self) -> &'static str {
        match self {
            ErrorCode::InvalidArgument => {
                "Input is missing or malformed; `details` names the argument and what it accepts."
            }
            ErrorCode::NotFound => {
                "A named package, tap, command, or document does not exist, or is not declared."
            }
            ErrorCode::PreconditionFailed => {
                "The machine or the config is not ready: mise is missing, the package is declared \
                 in a config file pkg does not edit, or removing it would also uninstall packages \
                 no config declares."
            }
            ErrorCode::Unavailable => {
                "The Homebrew or App Store catalog could not be reached; retry the same command."
            }
            ErrorCode::MiseFailed => {
                "mise reported a failure; `details.output` holds the end of its output, and \
                 anything pkg declared for the failed install was taken back out of the config."
            }
            ErrorCode::Internal => {
                "A defect in pkg itself, or a stdout that cannot take the result; `message` says \
                 which."
            }
        }
    }
}

/// A failed operation: what the person or agent needs to decide whether and how to recover.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub code: ErrorCode,
    pub message: String,
    /// Boxed so `Result<_, Failure>` stays small on the success path.
    pub details: Box<Value>,
    pub suggestions: Vec<String>,
}

impl Failure {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Failure {
        Failure {
            code,
            message: message.into(),
            details: Box::new(json!({})),
            suggestions: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_details(mut self, details: Value) -> Failure {
        self.details = Box::new(details);
        self
    }

    #[must_use]
    pub fn suggest(mut self, suggestion: impl Into<String>) -> Failure {
        self.suggestions.push(suggestion.into());
        self
    }

    pub fn invalid_argument(message: impl Into<String>) -> Failure {
        Failure::new(ErrorCode::InvalidArgument, message)
    }

    pub fn not_found(message: impl Into<String>) -> Failure {
        Failure::new(ErrorCode::NotFound, message)
    }

    pub fn precondition(message: impl Into<String>) -> Failure {
        Failure::new(ErrorCode::PreconditionFailed, message)
    }

    pub fn unavailable(message: impl Into<String>) -> Failure {
        Failure::new(ErrorCode::Unavailable, message)
    }

    pub fn internal(message: impl Into<String>) -> Failure {
        Failure::new(ErrorCode::Internal, message)
    }

    /// The document written to stderr with `--output json`; [`ErrorDocument`] is its schema.
    pub fn to_json(&self) -> Value {
        json!({
            "error": {
                "code": self.code.as_str(),
                "message": self.message,
                "retryable": self.code.retryable(),
                "details": self.details,
                "suggestions": self.suggestions,
            }
        })
    }

    /// The failure for people: the message, mise's own output when it failed, and suggestions.
    pub fn to_text(&self) -> String {
        let mut text = format!("error: {}\n", self.message);
        if let Some(output) = self.details.get("output").and_then(Value::as_array) {
            for line in output.iter().filter_map(Value::as_str) {
                text.push_str(&format!("  | {line}\n"));
            }
        }
        for suggestion in &self.suggestions {
            text.push_str(&format!("hint: {suggestion}\n"));
        }
        text
    }
}

/// What a failing command writes to stderr with `--output json`, as one JSON document.
#[derive(Serialize, JsonSchema)]
pub struct ErrorDocument {
    pub error: ErrorBody,
}

#[derive(Serialize, JsonSchema)]
pub struct ErrorBody {
    /// Stable code; branch on this, not on the message.
    pub code: ErrorCode,
    /// One sentence for a person.
    pub message: String,
    /// Whether running the same command again, unchanged, can succeed.
    pub retryable: bool,
    /// Facts about the failure, such as the argument, the package, the config file, or mise's
    /// output.
    pub details: Value,
    /// Commands or changes that recover from the failure, most useful first.
    pub suggestions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_document_carries_every_contract_field() {
        let document = Failure::not_found("no such formula")
            .with_details(json!({ "package": "brew:wgett" }))
            .suggest("pkg search wgett")
            .to_json();
        assert_eq!(document["error"]["code"], "NOT_FOUND");
        assert_eq!(document["error"]["retryable"], false);
        assert_eq!(document["error"]["details"]["package"], "brew:wgett");
        assert_eq!(document["error"]["suggestions"][0], "pkg search wgett");
    }

    #[test]
    fn only_an_unreachable_catalog_is_retryable() {
        let retryable: Vec<ErrorCode> =
            ErrorCode::ALL.into_iter().filter(|code| code.retryable()).collect();
        assert_eq!(retryable, [ErrorCode::Unavailable]);
    }

    #[test]
    fn text_shows_mises_output_and_the_hints() {
        let text = Failure::new(ErrorCode::MiseFailed, "mise could not install brew:wget")
            .with_details(json!({ "output": ["no bottle for this macOS"] }))
            .suggest("pkg info brew:wget")
            .to_text();
        assert_eq!(
            text,
            "error: mise could not install brew:wget\n  | no bottle for this macOS\nhint: pkg info brew:wget\n"
        );
    }
}
