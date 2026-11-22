//! Every declared bootstrap package and its installed state: mise's answer, except that Homebrew
//! casks are read from the Caskroom, because mise checks a cask over the network.

use std::collections::BTreeSet;

use crate::Error;
use crate::caskroom::Caskroom;
use crate::common::{Machine, joined};
use crate::mise::PackageState;

/// The manager whose check mise makes over the network.
const CASKS: &str = "brew-cask";

/// The managers mise chooses from for an unprefixed `[bootstrap.packages]` key when
/// `system_packages.prefer` is unset, in mise 2026.11.9 and later.
const UNPREFIXED: [&str; 9] =
    ["apt", "dnf", "pacman", "zypper", "apk", "brew", "winget", "scoop", "choco"];

impl Machine<'_> {
    /// Every `[bootstrap.packages]` entry with its installed state, in manager order, then config
    /// order. mise checks every manager but casks, which the Caskroom answers, unless the config's
    /// own settings leave casks to mise or the host has no Caskroom.
    ///
    /// # Errors
    ///
    /// `MISE_FAILED` when mise cannot report them or read its settings, and `PRECONDITION_FAILED`
    /// when a cask's directory cannot be read.
    pub(crate) fn package_states(&self) -> Result<Vec<PackageState>, Error> {
        let mise = &self.environment.mise;
        let declared = self.declarations.package_managers();
        let Some(caskroom) = Caskroom::of(self.environment.child_env, self.environment.home) else {
            return mise.package_states(None);
        };
        if !declared.contains(CASKS) {
            return mise.package_states(None);
        }
        let (managers, prefer) = std::thread::scope(|scope| {
            let managers = scope.spawn(|| mise.list_setting("system_packages.managers"));
            let prefer = scope.spawn(|| mise.list_setting("system_packages.prefer"));
            (joined(managers.join()), joined(prefer.join()))
        });
        let (managers, prefer) = (managers?, prefer?);
        let Some(allowed) = without_casks(&declared, managers.as_deref(), prefer.as_deref()) else {
            return mise.package_states(None);
        };
        let mut states = mise.package_states(Some(&allowed.join(",")))?;
        // Every manager mise reports but pkg did not name was left out by mistake: an unprefixed
        // key chose a manager pkg does not know. Only mise's own answer is then complete.
        let missed =
            |state: &PackageState| state.manager != CASKS && !allowed.contains(&state.manager);
        if managers.is_none() && states.iter().any(missed) {
            return mise.package_states(None);
        }
        for state in states.iter_mut().filter(|state| state.manager == CASKS) {
            caskroom.fill(state)?;
        }
        Ok(states)
    }
}

/// The managers mise may use while it leaves casks to pkg, given the managers the config's
/// `[bootstrap.packages]` keys name and mise's `system_packages.managers` and `prefer` settings.
/// `None` when mise keeps casks: none is declared, the config already leaves them out, an
/// unprefixed key may resolve to one, or no other manager would remain.
fn without_casks(
    declared: &BTreeSet<&str>,
    managers: Option<&[String]>,
    prefer: Option<&[String]>,
) -> Option<Vec<String>> {
    let names = |list: Option<&[String]>| -> BTreeSet<String> {
        list.unwrap_or_default().iter().cloned().collect()
    };
    let (configured, preferred) = (names(managers), names(prefer));
    let leaves_casks_out = managers.is_some() && !configured.contains(CASKS);
    if !declared.contains(CASKS) || leaves_casks_out || preferred.contains(CASKS) {
        return None;
    }
    let mut allowed = if managers.is_some() {
        configured
    } else {
        let mut every: BTreeSet<String> = preferred;
        every.extend(declared.iter().chain(&UNPREFIXED).map(|name| (*name).to_string()));
        every
    };
    allowed.remove(CASKS);
    (!allowed.is_empty()).then(|| allowed.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn mise_may_use_every_manager_a_key_names_or_an_unprefixed_key_may_choose() {
        let declared = BTreeSet::from(["brew", "brew-cask", "mas", "flatpak-user"]);
        assert_eq!(
            without_casks(&declared, None, None).unwrap(),
            names(&[
                "apk",
                "apt",
                "brew",
                "choco",
                "dnf",
                "flatpak-user",
                "mas",
                "pacman",
                "scoop",
                "winget",
                "zypper",
            ])
        );
    }

    #[test]
    fn a_preferred_manager_stays_available_for_unprefixed_keys() {
        let declared = BTreeSet::from(["brew-cask"]);
        let prefer = names(&["macports", "brew"]);
        let allowed = without_casks(&declared, None, Some(&prefer)).unwrap();
        assert!(allowed.contains(&"macports".to_string()), "{allowed:?}");
        assert!(!allowed.contains(&"brew-cask".to_string()), "{allowed:?}");
    }

    #[test]
    fn the_configured_managers_stay_the_only_ones_besides_casks() {
        let declared = BTreeSet::from(["brew", "brew-cask", "mas"]);
        let managers = names(&["mas", "brew-cask", "brew"]);
        assert_eq!(
            without_casks(&declared, Some(&managers), None).unwrap(),
            names(&["brew", "mas"])
        );
    }

    #[test]
    fn mise_keeps_casks_when_the_config_already_decides_about_them() {
        let declared = BTreeSet::from(["brew", "brew-cask"]);
        assert_eq!(without_casks(&BTreeSet::from(["brew", "mas"]), None, None), None);
        assert_eq!(without_casks(&declared, Some(&names(&["brew"])), None), None);
        assert_eq!(without_casks(&declared, None, Some(&names(&["brew-cask", "brew"]))), None);
        assert_eq!(without_casks(&declared, Some(&names(&["brew-cask"])), None), None);
    }
}
