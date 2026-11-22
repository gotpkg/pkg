use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;

use crate::declarations::{Declarations, same_file};
use crate::environment::{Environment, real_path};
use crate::mise::ConfigTarget;
use crate::package::Package;
use crate::{Error, ErrorDetails, Recovery, Settings};

/// Whether a modifying command made its change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ReceiptStatus {
    /// The change is made.
    Completed,
    /// Nothing changed; the receipt shows what running without --dry-run would do.
    DryRun,
}

pub(crate) struct Machine<'a> {
    pub environment: Environment<'a>,
    pub backends: Vec<String>,
    pub config_files: Vec<PathBuf>,
    pub declarations: Declarations,
}

impl<'a> Machine<'a> {
    pub(crate) fn load(settings: &'a Settings) -> Result<Self, Error> {
        let environment = Environment::new(settings);
        let backends = environment.mise.tool_backends()?;
        let config_files = environment.mise.config_files()?;
        let declarations = Declarations::load(&config_files)?;
        Ok(Self { environment, backends, config_files, declarations })
    }

    pub(crate) fn package(&self, name: &str) -> Package {
        Package::parse(name, &self.backends)
    }

    pub(crate) fn packages(&self, names: &[String]) -> Vec<Package> {
        let mut packages: Vec<Package> = Vec::new();
        for name in names {
            let package = self.package(name);
            if !packages.iter().any(|seen| seen.key == package.key) {
                packages.push(package);
            }
        }
        packages
    }

    pub(crate) fn declared_in(&self, package: &Package) -> Option<&Path> {
        if package.is_tool() {
            self.declarations.tool_file(&package.key)
        } else {
            self.declarations.package_file(&package.key)
        }
    }

    pub(crate) fn target(&self, tool: bool) -> Result<ConfigTarget, Error> {
        let setting = if tool { "tools" } else { "packages" };
        self.environment.mise.write_target(setting, &self.environment.global_config)
    }

    pub(crate) fn editable_declaration(&self, package: &Package) -> Result<ConfigTarget, Error> {
        let target = self.target(package.is_tool())?;
        let Some(declared) = self.declared_in(package) else {
            return Err(Error::not_found(format!("{package} is not in your mise config"))
                .with_details(ErrorDetails {
                    package: Some(package.key.clone()),
                    configs: Some(self.config_files.iter().map(|path| real_path(path)).collect()),
                    ..Default::default()
                })
                .recover(Recovery::ListPackages));
        };
        if !same_file(declared, &target.path) {
            return Err(Error::precondition(format!(
                "{package} is declared in {}, and pkg only edits {}",
                real_path(declared).display(),
                target.real_path.display()
            ))
            .with_details(ErrorDetails {
                package: Some(package.key.clone()),
                config: Some(real_path(declared)),
                editable_config: Some(target.real_path.clone()),
                ..Default::default()
            })
            .recover(Recovery::RemoveDeclaration {
                package: package.key.clone(),
                config: real_path(declared),
                shared: true,
            }));
        }
        Ok(target)
    }
}

pub(crate) fn absolute(path: &Path) -> PathBuf {
    real_path(path)
}

pub(crate) fn join_words(values: &[String]) -> String {
    match values {
        [] => String::new(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        many => format!("{}, and {}", many[..many.len() - 1].join(", "), many[many.len() - 1]),
    }
}

/// The result of a mise query run on a scoped thread; a panic there is an internal error.
pub(crate) fn joined<T>(result: std::thread::Result<Result<T, Error>>) -> Result<T, Error> {
    result.unwrap_or_else(|_| Err(Error::internal("a mise query panicked")))
}
