use crate::{Error, ErrorDetails, Recovery};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Apply,
    DryRun,
}

impl Mode {
    pub fn is_dry_run(self) -> bool {
        self == Self::DryRun
    }
}

#[derive(Debug)]
pub struct InstallRequest {
    pub packages: Vec<String>,
    pub mode: Mode,
}

#[derive(Debug)]
pub struct RemoveRequest {
    pub packages: Vec<String>,
    pub mode: Mode,
}

#[derive(Debug)]
pub struct UpdateRequest {
    /// Empty means every declared package and tool.
    pub packages: Vec<String>,
    pub mode: Mode,
}

#[derive(Debug, Default)]
pub struct ListRequest {
    pub managers: Vec<String>,
    /// None returns every entry.
    pub limit: Option<u64>,
    pub cursor: Option<String>,
}

/// A validated search query; construction does not need a machine context.
#[derive(Debug)]
pub struct SearchRequest {
    pub(crate) query: String,
    pub(crate) terms: Vec<String>,
    pub(crate) managers: Vec<SearchSource>,
    pub(crate) limit: u64,
    pub(crate) cursor: Option<String>,
}

impl SearchRequest {
    /// Parses the query once, before the caller resolves an execution context.
    ///
    /// # Errors
    /// Returns `InvalidArgument` for an empty/stop-word-only query or a limit outside 1..=50.
    pub fn new(
        query: String,
        managers: Vec<SearchSource>,
        limit: u64,
        cursor: Option<String>,
    ) -> Result<Self, Error> {
        validate_limit(limit, Some(50))?;
        let terms = crate::matching::query_words(&query);
        if terms.is_empty() {
            return Err(Error::invalid_argument("the query has no words to search for")
                .recover(Recovery::Search("ffmpeg".into())));
        }
        Ok(Self { query, terms, managers, limit, cursor })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchSource {
    Brew,
    Cask,
    Tool,
    AppStore,
}

impl SearchSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Brew => "brew",
            Self::Cask => "brew-cask",
            Self::Tool => "tool",
            Self::AppStore => "mas",
        }
    }
}

#[derive(Debug)]
pub struct InfoRequest {
    pub package: String,
}

#[derive(Debug)]
pub struct LocateRequest {
    pub package: String,
}

#[derive(Debug)]
pub struct SyncRequest {
    pub mode: Mode,
}

#[derive(Debug)]
pub struct PruneRequest {
    pub mode: Mode,
    pub force: bool,
}

#[derive(Debug)]
pub struct TapAddRequest {
    pub tap: String,
    pub url: String,
    pub mode: Mode,
}

#[derive(Debug)]
pub struct TapRemoveRequest {
    pub tap: String,
    pub mode: Mode,
}

pub(crate) fn validate_packages(packages: &[String], required: bool) -> Result<(), Error> {
    if required && packages.is_empty() {
        return Err(Error::invalid_argument("at least one package is required"));
    }
    for package in packages {
        validate_package(package)?;
    }
    Ok(())
}

pub(crate) fn validate_package(package: &str) -> Result<(), Error> {
    if !crate::package::is_package(package) || package.starts_with('-') || package.contains('\0') {
        return Err(Error::invalid_argument(format!("`{package}` is not a package name"))
            .with_details(ErrorDetails { package: Some(package.into()), ..Default::default() }));
    }
    Ok(())
}

pub(crate) fn validate_limit(limit: u64, max: Option<u64>) -> Result<(), Error> {
    if limit == 0 || max.is_some_and(|max| limit > max) {
        return Err(Error::invalid_argument("limit is outside the accepted range").with_details(
            ErrorDetails {
                argument: Some("limit".into()),
                value: Some(limit.to_string()),
                ..Default::default()
            },
        ));
    }
    Ok(())
}

pub(crate) fn validate_operand(value: &str, argument: &str) -> Result<(), Error> {
    if value.is_empty() || value.starts_with('-') || value.contains('\0') {
        return Err(Error::invalid_argument(format!("`{value}` is not a valid {argument}"))
            .with_details(ErrorDetails {
                argument: Some(argument.into()),
                value: Some(value.into()),
                ..Default::default()
            }));
    }
    Ok(())
}
