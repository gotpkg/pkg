//! A parsed command line, as a command's `run` function sees it.

use clap::ArgMatches;

use crate::environment::Environment;
use crate::failure::Failure;
use crate::registry::CommandSpec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

pub struct Invocation<'a> {
    pub spec: &'static CommandSpec,
    pub output: OutputFormat,
    matches: &'a ArgMatches,
}

impl<'a> Invocation<'a> {
    pub fn new(
        spec: &'static CommandSpec,
        matches: &'a ArgMatches,
        output: OutputFormat,
    ) -> Invocation<'a> {
        Invocation { spec, output, matches }
    }

    pub fn string(&self, name: &str) -> Option<String> {
        self.matches.get_one::<String>(name).cloned()
    }

    pub fn strings(&self, name: &str) -> Vec<String> {
        self.matches
            .get_many::<String>(name)
            .map(|values| values.cloned().collect())
            .unwrap_or_default()
    }

    pub fn integer(&self, name: &str) -> Option<u64> {
        self.matches.get_one::<u64>(name).copied()
    }

    pub fn flag(&self, name: &str) -> bool {
        self.matches.get_flag(name)
    }

    pub fn dry_run(&self) -> bool {
        self.spec.dry_run && self.flag("dry-run")
    }

    /// Where mise, the catalogs, and the cache are, from the process environment.
    ///
    /// # Errors
    ///
    /// `PRECONDITION_FAILED` when no home directory is set.
    pub fn environment(&self) -> Result<Environment, Failure> {
        Environment::from_process()
    }
}
