use std::fmt;
use std::path::PathBuf;

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidArgument,
    NotFound,
    PreconditionFailed,
    Unavailable,
    MiseFailed,
    Internal,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    pub details: Box<ErrorDetails>,
    pub recovery: Vec<Recovery>,
}

/// Optional facts are absent when the operation did not observe them.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packages: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed: Option<Vec<String>>,
    /// Declaration removals confirmed before a later step failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configs: Option<Vec<PathBuf>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editable_config: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undeclared: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Outer None means no process status was observed; Some(None) means a signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Option<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Vec<String>>,
    /// Whether these diagnostics were also delivered to the progress callback.
    #[serde(skip)]
    pub output_streamed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argument: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// A file or directory the failure concerns, other than a config file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// The step of `setup` or `bootstrap` that failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    /// What the operation completed before it failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<Vec<String>>,
    /// Whether `mise bootstrap` had started, so that changes it made may remain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changes_may_remain: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Recovery {
    InstallMise,
    SelectMise,
    CheckMiseVersion,
    RetryNetwork,
    ListPackages,
    Search(String),
    Info(String),
    Install(String),
    PreviewPrune,
    Prune,
    Keep(String),
    RemoveDeclaration {
        package: String,
        config: PathBuf,
        shared: bool,
    },
    ListTaps,
    RemoveTap {
        tap: String,
        config: PathBuf,
        editable_config: PathBuf,
    },
    Cursor,
    /// `mise self-update`.
    UpdateMise,
    /// Update mise with the package manager that installed it.
    UpdateMiseWithItsPackageManager,
    /// Move this file or directory aside, then run the command again.
    MoveAside(PathBuf),
    /// Run the command in a terminal, where mise can ask for what it needs.
    RunInTerminal,
    /// Fix the cause and run the same command again.
    RunAgain,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into(), details: Box::default(), recovery: Vec::new() }
    }

    pub(crate) fn with_details(mut self, details: ErrorDetails) -> Self {
        *self.details = details;
        self
    }

    pub(crate) fn recover(mut self, recovery: Recovery) -> Self {
        self.recovery.push(recovery);
        self
    }

    pub(crate) fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidArgument, message)
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, message)
    }

    pub(crate) fn precondition(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::PreconditionFailed, message)
    }

    pub(crate) fn unavailable(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unavailable, message)
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }

    pub fn retryable(&self) -> bool {
        self.kind == ErrorKind::Unavailable
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}
