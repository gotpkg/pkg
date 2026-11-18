//! Reading the plans mise prints for prune and upgrade, which have no JSON form.
//!
//! The parsers recognize only the lines they need and skip every other line, so a line mise adds
//! in a later release is ignored rather than misread.

/// The packages a prune removes, from its `remove brew:pkgconf@3.0.10` lines, as `(key, version)`.
pub(crate) fn removals(output: &str) -> Vec<(String, Option<String>)> {
    output
        .lines()
        .filter_map(|line| line.trim().strip_prefix("remove "))
        .filter_map(|package| package.split_whitespace().next())
        .filter(|package| package.contains(':'))
        .map(|package| {
            let (manager, rest) = package.split_once(':').unwrap_or(("", package));
            // `brew:postgresql@17@17.6`: the last `@` starts the version.
            match rest.rsplit_once('@').filter(|(name, _)| !name.is_empty()) {
                Some((name, version)) => (format!("{manager}:{name}"), Some(version.to_string())),
                None => (package.to_string(), None),
            }
        })
        .collect()
}

/// What `mise bootstrap packages upgrade --dry-run` would do to Homebrew packages.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct UpgradePlan {
    pub upgrades: Vec<PlannedUpgrade>,
    /// Casks mise leaves alone, such as apps that update themselves, with mise's reason.
    pub skipped: Vec<(String, String)>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PlannedUpgrade {
    /// `brew` or `brew-cask`.
    pub manager: &'static str,
    /// The formula's short name (`tg` for `alyraffauf/tap/tg`) or the cask's token.
    pub name: String,
    pub version: String,
}

/// Reads the upgrade plan. Formulae appear as `pour <name>/<version> (requested)` or
/// `build <name>/<version> from source (requested, ...)`; dependencies say `(dependency)` and are
/// not upgrades of a declared package. Casks appear as `install cask <token>/<version>` and
/// `mise brew-cask:<token>: skipped: <reason>`.
pub(crate) fn upgrade_plan(output: &str) -> UpgradePlan {
    let mut plan = UpgradePlan::default();
    for line in output.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("install cask ") {
            if let Some((name, version)) = rest.split_whitespace().next().and_then(name_version) {
                // A cask version can carry a build hash after a comma.
                let version = version.split(',').next().unwrap_or(&version).to_string();
                plan.upgrades.push(PlannedUpgrade { manager: "brew-cask", name, version });
            }
        } else if let Some(rest) =
            line.strip_prefix("pour ").or_else(|| line.strip_prefix("build "))
        {
            if !line.contains("(requested") {
                continue;
            }
            if let Some((name, version)) = rest.split_whitespace().next().and_then(name_version) {
                plan.upgrades.push(PlannedUpgrade { manager: "brew", name, version });
            }
        } else if let Some(rest) = line.strip_prefix("mise brew-cask:")
            && let Some((token, reason)) = rest.split_once(": skipped: ")
        {
            plan.skipped.push((format!("brew-cask:{token}"), reason.to_string()));
        }
    }
    plan
}

fn name_version(word: &str) -> Option<(String, String)> {
    word.split_once('/')
        .filter(|(name, version)| !name.is_empty() && !version.is_empty())
        .map(|(name, version)| (name.to_string(), version.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removals_read_the_remove_lines() {
        let output = "mise WARN something\nremove brew:brightness@1.2\nremove brew:postgresql@17@17.6\nmise brew-cask: nothing to prune\n";
        assert_eq!(
            removals(output),
            [
                ("brew:brightness".to_string(), Some("1.2".to_string())),
                ("brew:postgresql@17".to_string(), Some("17.6".to_string())),
            ]
        );
        assert!(removals("mise brew-cask: nothing to prune").is_empty());
    }

    #[test]
    fn the_upgrade_plan_keeps_requested_packages_and_skips() {
        let output = "\
pour dolt/2.7.4 (dependency)
pour imagemagick/7.1.2-36 (requested)
build tg/0.10.0 from source (requested, source-only formula, no bottles)
mise brew-cask:1password: skipped: installed app is running and updates itself
mise brew-cask:figma: already up to date
install cask claude/6.6395.0,d71a20056be3b25708751e98bef60759ea1f441b
link app /Applications/Claude.app
mas upgrade 1365531024 1569813296
";
        let plan = upgrade_plan(output);
        let upgrades: Vec<(&str, &str, &str)> = plan
            .upgrades
            .iter()
            .map(|upgrade| (upgrade.manager, upgrade.name.as_str(), upgrade.version.as_str()))
            .collect();
        assert_eq!(
            upgrades,
            [
                ("brew", "imagemagick", "7.1.2-36"),
                ("brew", "tg", "0.10.0"),
                ("brew-cask", "claude", "6.6395.0"),
            ]
        );
        assert_eq!(
            plan.skipped,
            [("brew-cask:1password".into(), "installed app is running and updates itself".into())]
        );
    }
}
