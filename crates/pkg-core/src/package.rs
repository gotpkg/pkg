//! Packages as mise names them, and whether mise installs one as a bootstrap package or a tool.
//!
//! `brew:wget`, `brew-cask:firefox` and `mas:497799835` are bootstrap packages, declared in
//! `[bootstrap.packages]`. `npm:prettier`, `github:cli/cli` and a bare registry name such as
//! `ripgrep` are tools, declared in `[tools]`. What tells them apart is the prefix: a prefix that
//! names one of mise's tool backends (`mise backends ls`) is a tool, and so is no prefix at all.

use std::fmt;

/// A package as written on the command line, with its version split off where mise reads one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Package {
    /// The config key: `brew:postgresql@17`, `apt:curl`, `npm:@vue/language-server`, `ripgrep`.
    pub key: String,
    /// The requested version, when the command line names one: `20` in `node@20`. Homebrew keeps
    /// `@` in its formula and cask names, so `brew:` and `brew-cask:` packages never have one.
    pub version: Option<String>,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A `[bootstrap.packages]` entry, installed by `manager`.
    Bootstrap { manager: String },
    /// A `[tools]` entry; `backend` is `None` for a registry name such as `ripgrep`.
    Tool { backend: Option<String> },
}

/// Whether `value` can name a package: an optional lowercase `prefix:` and no whitespace.
pub fn is_package(value: &str) -> bool {
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return false;
    }
    match value.split_once(':') {
        Some((prefix, rest)) => is_prefix(prefix) && !rest.is_empty(),
        None => true,
    }
}

fn is_prefix(prefix: &str) -> bool {
    prefix.chars().next().is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && prefix.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

impl Package {
    /// Reads `value`, which the parser has checked with [`is_package`]. `tool_backends` is mise's
    /// list of tool backends; any other prefix is a bootstrap package manager.
    pub(crate) fn parse(value: &str, tool_backends: &[String]) -> Package {
        let (prefix, rest) = match value.split_once(':') {
            Some((prefix, rest)) => (Some(prefix), rest),
            None => (None, value),
        };
        let kind = match prefix {
            Some(prefix) if !tool_backends.iter().any(|backend| backend == prefix) => {
                Kind::Bootstrap { manager: prefix.to_string() }
            }
            prefix => Kind::Tool { backend: prefix.map(str::to_string) },
        };
        let homebrew = matches!(&kind, Kind::Bootstrap { manager } if manager.starts_with("brew"));
        // A leading `@` is part of an npm scope, not a version: `npm:@vue/language-server`.
        let at = rest.get(1..).and_then(|tail| tail.rfind('@')).map(|index| index + 1);
        let (name, version) = match at {
            Some(index) if !homebrew => (&rest[..index], Some(rest[index + 1..].to_string())),
            _ => (rest, None),
        };
        let key = match prefix {
            Some(prefix) => format!("{prefix}:{name}"),
            None => name.to_string(),
        };
        Package { key, version: version.filter(|version| !version.is_empty()), kind }
    }

    pub(crate) fn is_tool(&self) -> bool {
        matches!(self.kind, Kind::Tool { .. })
    }

    /// The bootstrap package manager, such as `brew`; `None` for a tool.
    pub(crate) fn manager(&self) -> Option<&str> {
        match &self.kind {
            Kind::Bootstrap { manager } => Some(manager),
            Kind::Tool { .. } => None,
        }
    }

    /// The name after the prefix: `wget` for `brew:wget`, `ripgrep` for `ripgrep`.
    pub(crate) fn name(&self) -> &str {
        self.key.split_once(':').map_or(self.key.as_str(), |(_, name)| name)
    }

    /// The argument mise takes to install it: the key with its version, `@latest` for a tool
    /// without one, so the config records `"latest"`.
    pub(crate) fn install_argument(&self) -> String {
        match (&self.version, self.is_tool()) {
            (Some(version), _) => format!("{}@{version}", self.key),
            (None, true) => format!("{}@latest", self.key),
            (None, false) => self.key.clone(),
        }
    }
}

impl fmt::Display for Package {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backends() -> Vec<String> {
        ["aqua", "cargo", "github", "npm"].map(str::to_string).to_vec()
    }

    #[test]
    fn a_prefix_that_is_not_a_tool_backend_is_a_bootstrap_manager() {
        let wget = Package::parse("brew:wget", &backends());
        assert_eq!(wget.kind, Kind::Bootstrap { manager: "brew".into() });
        assert_eq!(wget.install_argument(), "brew:wget");
        let prettier = Package::parse("npm:prettier", &backends());
        assert_eq!(prettier.kind, Kind::Tool { backend: Some("npm".into()) });
        assert_eq!(prettier.install_argument(), "npm:prettier@latest");
        let ripgrep = Package::parse("ripgrep", &backends());
        assert_eq!(ripgrep.kind, Kind::Tool { backend: None });
    }

    #[test]
    fn homebrew_keeps_its_at_sign_and_others_split_off_a_version() {
        let postgres = Package::parse("brew:postgresql@17", &backends());
        assert_eq!((postgres.key.as_str(), postgres.version), ("brew:postgresql@17", None));
        let curl = Package::parse("apt:curl@8.5.0-2", &backends());
        assert_eq!((curl.key.as_str(), curl.version.as_deref()), ("apt:curl", Some("8.5.0-2")));
        let node = Package::parse("node@20", &backends());
        assert_eq!((node.key.as_str(), node.install_argument().as_str()), ("node", "node@20"));
        let vue = Package::parse("npm:@vue/language-server", &backends());
        assert_eq!((vue.key.as_str(), vue.version), ("npm:@vue/language-server", None));
        let pinned = Package::parse("npm:@vue/language-server@3.0.0", &backends());
        assert_eq!(pinned.key, "npm:@vue/language-server");
        assert_eq!(pinned.version.as_deref(), Some("3.0.0"));
    }

    #[test]
    fn names_need_a_lowercase_prefix_and_no_whitespace() {
        for good in ["brew:wget", "ripgrep", "github:cli/cli", "brew-cask:steipete/tap/codexbar"] {
            assert!(is_package(good), "{good}");
        }
        for bad in ["", "brew:", ":wget", "Brew:wget", "brew wget", "a b"] {
            assert!(!is_package(bad), "{bad}");
        }
    }
}
