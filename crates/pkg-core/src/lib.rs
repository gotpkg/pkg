//! Typed, blocking package operations backed by mise and the Homebrew/App Store catalogs.
//!
//! # Context and effects
//!
//! Construct a `Client` from `Settings`: absolute home, global configuration and cache paths,
//! a mise executable, catalog URLs, and an explicit child-environment snapshot. Construction
//! validates settings without running mise. Each operation reads current declarations and
//! installation state; the client is not an inventory snapshot. Child processes receive only
//! that snapshot plus the explicit home/configuration selection, with stdin closed, except that
//! `bootstrap` with `terminal: true` lets mise read the caller's stdin and write to its stderr.
//! The host's environment, current directory, signal policy, and standard streams are never
//! changed.
//!
//! The caller supplies credentials through the child environment, which is omitted from Debug
//! output. This is process isolation, not a sandbox: mise and the selected packages still run
//! with the caller's permissions. Mutations of the same configuration must be serialized across
//! clients and processes. A client can move to a worker thread; its methods require mutable
//! access and block until their subprocesses have been reaped.
//!
//! Catalog proxy selection uses the supplied environment snapshot too, including `NO_PROXY`;
//! it never implicitly selects the host process's proxy for a different context.
//!
//! # Operations
//!
//! `Client` exposes `install`, `remove`, `list`, `search`, `info`, `outdated`, `update`, `sync`,
//! `prune`, `locate`, `tap_list`, `tap_add`, `tap_remove`, `context`, `setup`, and `bootstrap`.
//! Requests are typed and validated before I/O: package names use mise syntax; install/remove
//! need at least one name; pagination limits must be positive; search accepts the four catalog
//! source variants and at most 50 results. Empty update selection means every declared
//! package/tool. Construct search requests with `SearchRequest::new`, which validates and
//! tokenizes the query before context resolution. Package/tap operands cannot begin with an
//! option prefix.
//!
//! `setup` and `bootstrap` get a machine ready rather than changing packages: they may download
//! and run mise's installer script (`Settings::mise_installer`) with `sh`, edit or move aside
//! pkg's block of `PATH` lines in shell startup files, and replace `~/.local/bin/pkg` with a link
//! to mise's shim. `bootstrap` runs `mise bootstrap`, which applies whatever the configuration
//! declares and is not a transaction. Both support macOS and Linux only.
//!
//! Mutation requests choose `Mode::Apply` or `Mode::DryRun` explicitly. A preview uses the same
//! decisions and refusals as apply without installing, uninstalling, or writing configuration.
//! No saved plan can be executed later without rereading state. Reads may update only the
//! documented catalog search cache. Package routing, write-target ownership, install rollback,
//! prune protection, ordering, and pagination retain the CLI's existing package contract.
//!
//! Each operation returns `Result<Outcome<T>, Error>`. `Outcome` contains the typed result and
//! retained warnings, so ignoring progress loses no outcome information. Results describe
//! observed actions rather than echoing requests; progress does not certify completion.
//! Recoverable failures expose `ErrorKind`, typed `ErrorDetails`, and semantic `Recovery`
//! actions, not CLI command hints or a JSON value. Partial-install errors identify the failed
//! package and preceding completed packages; configuration refusals identify the conflicting
//! files. Native paths remain paths until the frontend formats them.
//!
//! # Progress
//!
//! Each operation borrows a progress callback for that call only. `Progress::Output` identifies
//! stdout or stderr and borrows the received text; `Progress::Warning` borrows a warning. Calls
//! are serialized on the operation's calling thread, preserve each stream's order, and end
//! before the method returns. There is no ordering promise between separate streams. Internal
//! pipe readers use a bounded handoff; the library does not keep a second progress history.
//! Callbacks must return promptly, must not reenter the same client, and must not panic.
//!
//! The API supplies no cancellation or rollback-on-drop guarantee. A failed mutation may have
//! completed earlier work, reported in its error. It never exits the host process or prompts.
//!
//! # Compatibility
//!
//! This crate replaces the unpublished `pkg` implementation-module API. The executable keeps
//! its name, CLI contracts, and mise-owned state. No UniFFI types, async runtime, daemon, or
//! second package inventory are required. CLI parsing, rendering, discovery, and exit statuses
//! belong to the executable crate, not this API.

mod catalog;
mod client;
mod common;
mod context;
mod declarations;
mod environment;
mod error;
mod info;
mod install;
mod list;
mod locate;
pub mod matching;
mod mise;
mod onboarding;
mod outdated;
mod package;
mod progress;
mod prune;
mod remove;
mod requests;
mod search;
mod settings;
mod startup;
mod sync;
mod tap;
mod update;

pub use client::Client;
pub use common::ReceiptStatus;
pub use context::{Catalogs, ConfigContext, Context, MiseContext, PkgTool};
pub use error::{Error, ErrorDetails, ErrorKind, Recovery};
pub use info::PackageInfo;
pub use install::{InstallAction, InstallKind, InstallReceipt, Installed};
pub use list::Listed;
pub use locate::Location;
pub use onboarding::bootstrap::{BootstrapReceipt, SourceKind, SourceReport};
pub use onboarding::handoff::{Declaration, Handoff, Link, PkgOwner, pkg_owner};
pub use onboarding::mise::{MiseAction, MiseSetup};
pub use onboarding::setup::SetupReceipt;
pub use onboarding::shims::{PathAction, PathChange};
pub use onboarding::startup_files::StartupFiles;
pub use outdated::{OutdatedPackage, OutdatedReport, Skipped};
pub use package::is_package;
pub use progress::{Outcome, OutputStream, Progress};
pub use prune::PruneReceipt;
pub use remove::{LeftInstalled, RemoveReceipt, Removed};
pub use requests::{
    BootstrapRequest, BootstrapSource, InfoRequest, InstallRequest, ListRequest, LocateRequest,
    Mode, PruneRequest, RemoveRequest, SearchRequest, SearchSource, SetupRequest, SyncRequest,
    TapAddRequest, TapRemoveRequest, UpdateRequest,
};
pub use search::Found;
pub use settings::{
    DEFAULT_APP_STORE_API, DEFAULT_HOMEBREW_API, DEFAULT_MISE_INSTALLER, Settings, find_on_path,
};
pub use sync::SyncReceipt;
pub use tap::{TapEntry, TapList, TapReceipt};
pub use update::{UpdateReceipt, Updated};
