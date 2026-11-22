//! Whether a shim refuses an invocation, and what it suggests instead: a pure function of the
//! shim, its arguments, and whether a virtual environment is active.

use super::catalog::{self, Action, Form, Names, Rule, Tool};

/// One run of a shim.
pub(crate) struct Call<'a> {
    pub(crate) tool: &'a str,
    pub(crate) args: &'a [String],
    /// `VIRTUAL_ENV` or `CONDA_PREFIX` is set.
    pub(crate) in_virtual_environment: bool,
}

pub(crate) enum Verdict {
    /// Run the real tool.
    Pass,
    Refuse(Refusal),
}

/// What to tell the reader instead of running the tool.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Refusal {
    pub(crate) tool: String,
    pub(crate) action: Action,
    /// The complete `pkg` command: `pkg install brew:wget`.
    pub(crate) command: String,
    /// The same names under another manager that may hold them.
    pub(crate) alternative: Option<String>,
}

/// The shim's arguments split into what the rules read.
struct Parsed<'a> {
    /// Arguments that are not options, in order: the verb and the names.
    words: Vec<&'a str>,
    /// Every option, without an inline `=value`.
    options: Vec<&'a str>,
    /// Values of the tool's name options (`--id`).
    named: Vec<&'a str>,
    /// An option that takes a separate value was used.
    separate_value: bool,
}

pub(crate) fn decide(call: &Call) -> Verdict {
    let Some(tool) = catalog::find(call.tool) else { return Verdict::Pass };
    let parsed = parse(tool, call.args);
    for rule in tool.rules {
        if let Some(candidates) = candidates(rule, &parsed)
            && applies(rule, call, &parsed)
        {
            return refusal(tool, rule, &parsed, &candidates);
        }
    }
    Verdict::Pass
}

fn parse<'a>(tool: &Tool, args: &'a [String]) -> Parsed<'a> {
    let mut parsed =
        Parsed { words: Vec::new(), options: Vec::new(), named: Vec::new(), separate_value: false };
    let mut rest = args.iter().map(String::as_str);
    while let Some(argument) = rest.next() {
        if argument == "--" {
            parsed.words.extend(rest.by_ref());
            break;
        }
        if !argument.starts_with('-') || argument == "-" {
            parsed.words.push(argument);
            continue;
        }
        let (option, inline) = match argument.split_once('=') {
            Some((option, value)) => (option, Some(value)),
            None => (argument, None),
        };
        parsed.options.push(option);
        if tool.name_options.contains(&option) {
            parsed.named.extend(inline.or_else(|| rest.next()));
        } else if inline.is_none() && tool.value_options.contains(&option) {
            parsed.separate_value = true;
            rest.next();
        }
    }
    parsed
}

/// The words a rule would read as names, when the invocation has the rule's verb.
fn candidates<'a>(rule: &Rule, parsed: &Parsed<'a>) -> Option<Vec<&'a str>> {
    match rule.form {
        Form::Verb { prefix, verbs } => {
            let (head, names) = parsed.words.split_at_checked(prefix.len() + 1)?;
            let (verb, leading) = head.split_last()?;
            (leading == prefix && verbs.contains(verb)).then(|| names.to_vec())
        }
        Form::Sync => is_sync_install(&parsed.options).then(|| parsed.words.clone()),
    }
}

/// pacman's `-S`, with or without `y` and `u`, and not a search, info, list, group or clean.
fn is_sync_install(options: &[&str]) -> bool {
    let mut sync = false;
    for option in options {
        match *option {
            "--sync" => sync = true,
            "--search" | "--info" | "--list" | "--groups" | "--clean" => return false,
            _ => {
                let Some(letters) = option.strip_prefix("-S").filter(|_| !option.starts_with("--"))
                else {
                    continue;
                };
                if letters.contains(['s', 'i', 'l', 'g', 'c']) {
                    return false;
                }
                sync = true;
            }
        }
    }
    sync
}

fn applies(rule: &Rule, call: &Call, parsed: &Parsed) -> bool {
    let has = |options: &[&str]| options.iter().any(|option| parsed.options.contains(option));
    (rule.requires_any.is_empty() || has(rule.requires_any))
        && !has(rule.unless_any)
        && !(rule.outside_virtual_environment && call.in_virtual_environment)
        && !parsed.separate_value
}

fn refusal(tool: &Tool, rule: &Rule, parsed: &Parsed, candidates: &[&str]) -> Verdict {
    let names = if parsed.named.is_empty() { candidates } else { parsed.named.as_slice() };
    let names = match rule.names {
        Names::AfterRemote => {
            let packages = match names.split_first() {
                Some((remote, packages)) if !packages.is_empty() && !remote.contains('.') => {
                    packages
                }
                _ => names,
            };
            if !packages.iter().all(|package| package.contains('.')) {
                return Verdict::Pass;
            }
            packages
        }
        Names::Plain | Names::Versioned => names,
    };
    if names.is_empty() || !names.iter().all(|name| is_plausible_name(name)) {
        return Verdict::Pass;
    }
    let mut packages = Vec::new();
    for name in names {
        match rule.names {
            Names::Versioned => match name.strip_suffix("@latest") {
                Some(module) => packages.push(module.to_string()),
                None if name.contains('@') => packages.push((*name).to_string()),
                None => return Verdict::Pass,
            },
            Names::Plain | Names::AfterRemote => {
                let name = match rule.target.version_separator {
                    Some(separator) => name.replacen(separator, "@", 1),
                    None => (*name).to_string(),
                };
                packages.push(name);
            }
        }
    }
    let manager = rule
        .target
        .variant
        .filter(|(options, _)| options.iter().any(|option| parsed.options.contains(option)))
        .map_or(rule.target.manager, |(_, manager)| manager);
    let command = |manager: &str| {
        let packages: Vec<String> =
            packages
                .iter()
                .map(|package| {
                    if manager.is_empty() {
                        package.clone()
                    } else {
                        format!("{manager}:{package}")
                    }
                })
                .collect();
        format!("pkg {} {}", rule.action.pkg_command(), packages.join(" "))
    };
    Verdict::Refuse(Refusal {
        tool: tool.name.to_string(),
        action: rule.action,
        command: command(manager),
        alternative: rule.target.also.map(command),
    })
}

/// A name `pkg` could take: not a path, a URL or an archive, and starting like a package name.
fn is_plausible_name(name: &str) -> bool {
    const ARCHIVES: [&str; 6] = [".tgz", ".tar.gz", ".zip", ".deb", ".rpm", ".whl"];
    let mut characters = name.chars();
    let starts_like_a_package =
        characters.next().is_some_and(|first| first.is_ascii_alphanumeric() || first == '@');
    let drive_path =
        name.chars().nth(1) == Some(':') && matches!(name.chars().nth(2), Some('\\' | '/'));
    starts_like_a_package
        && !drive_path
        && !name.contains("://")
        && !ARCHIVES.iter().any(|suffix| name.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The command the shim suggests for `tool args`, or `None` when it runs the real tool.
    fn suggestion(tool: &str, args: &str) -> Option<String> {
        suggestion_in(tool, args, false)
    }

    fn suggestion_in(tool: &str, args: &str, in_virtual_environment: bool) -> Option<String> {
        let args: Vec<String> = args.split_whitespace().map(str::to_string).collect();
        match decide(&Call { tool, args: &args, in_virtual_environment }) {
            Verdict::Pass => None,
            Verdict::Refuse(refusal) => Some(refusal.command),
        }
    }

    fn refuses(tool: &str, args: &str, command: &str) {
        assert_eq!(suggestion(tool, args).as_deref(), Some(command), "{tool} {args}");
    }

    fn passes(tool: &str, args: &str) {
        assert_eq!(suggestion(tool, args), None, "{tool} {args}");
    }

    #[test]
    fn homebrew_installs_and_removals_name_formulae_and_casks() {
        refuses("brew", "install wget", "pkg install brew:wget");
        refuses("brew", "install --cask iina vlc", "pkg install brew-cask:iina brew-cask:vlc");
        refuses("brew", "install owner/tap/tool", "pkg install brew:owner/tap/tool");
        refuses("brew", "uninstall wget", "pkg remove brew:wget");
        refuses("brew", "rm --cask iina", "pkg remove brew-cask:iina");
        passes("brew", "upgrade wget");
        passes("brew", "install");
        passes("brew", "--version");
    }

    #[test]
    fn linux_managers_name_their_own_prefix() {
        refuses("apt", "install -y curl", "pkg install apt:curl");
        refuses(
            "apt-get",
            "install --no-install-recommends curl=8.5.0-2",
            "pkg install apt:curl@8.5.0-2",
        );
        refuses("dnf", "install -y openssl-devel", "pkg install dnf:openssl-devel");
        refuses("zypper", "-n in git", "pkg install zypper:git");
        refuses("apk", "add --no-cache curl", "pkg install apk:curl");
        refuses("snap", "install --classic code", "pkg install snap:code");
        passes("apt", "update");
        passes("apt", "remove curl");
        passes("apt", "install ./local.deb");
        passes("apt", "install -t bookworm-backports curl");
        passes("dnf", "remove git");
        passes("apk", "add --virtual .build-deps gcc");
    }

    #[test]
    fn pacman_and_the_aur_helpers_refuse_syncs_that_name_packages() {
        refuses("pacman", "-S --noconfirm git", "pkg install pacman:git");
        refuses("pacman", "-Syu git vim", "pkg install pacman:git pacman:vim");
        refuses("pacman", "--sync git", "pkg install pacman:git");
        passes("pacman", "-Ss git");
        passes("pacman", "-Si git");
        passes("pacman", "-Sc");
        passes("pacman", "-Syu");
        passes("pacman", "-Rns git");
        passes("pacman", "-Q");
        refuses("yay", "-S google-chrome", "pkg install aur:google-chrome");
        refuses(
            "paru",
            "-S --needed visual-studio-code-bin",
            "pkg install aur:visual-studio-code-bin",
        );
        passes("yay", "-Ss chrome");
    }

    #[test]
    fn the_aur_helpers_also_name_the_official_repositories() {
        let args = vec!["-S".to_string(), "firefox".to_string()];
        let Verdict::Refuse(refusal) =
            decide(&Call { tool: "yay", args: &args, in_virtual_environment: false })
        else {
            panic!("yay -S firefox ran");
        };
        assert_eq!(refusal.alternative.as_deref(), Some("pkg install pacman:firefox"));
    }

    #[test]
    fn flatpak_reads_the_remote_and_the_installation() {
        refuses(
            "flatpak",
            "install flathub org.mozilla.firefox",
            "pkg install flatpak:org.mozilla.firefox",
        );
        refuses(
            "flatpak",
            "install org.mozilla.firefox",
            "pkg install flatpak:org.mozilla.firefox",
        );
        refuses(
            "flatpak",
            "install --user flathub org.mozilla.firefox org.gnome.Calculator",
            "pkg install flatpak-user:org.mozilla.firefox flatpak-user:org.gnome.Calculator",
        );
        passes("flatpak", "install https://dl.flathub.org/repo/appstream/x.flatpakref");
        passes("flatpak", "install flathub");
    }

    #[test]
    fn macos_and_windows_managers() {
        refuses("mas", "install 497799835", "pkg install mas:497799835");
        refuses("port", "install ffmpeg", "pkg install macports:ffmpeg");
        passes("port", "install ffmpeg +universal");
        refuses("nix", "profile install nixpkgs#ripgrep", "pkg install nix:nixpkgs#ripgrep");
        refuses("nix", "profile add nixpkgs#jq", "pkg install nix:nixpkgs#jq");
        passes("nix", "build nixpkgs#jq");
        refuses("winget", "install Microsoft.PowerToys", "pkg install winget:Microsoft.PowerToys");
        refuses("winget", "install -e --id Git.Git", "pkg install winget:Git.Git");
        refuses("winget", "install --id=Git.Git", "pkg install winget:Git.Git");
        passes("winget", "install --source msstore 9NBLGGH4NNS1");
        passes("winget", "list");
        refuses("scoop", "install extras/vlc", "pkg install scoop:extras/vlc");
        refuses("choco", "install -y git", "pkg install choco:git");
    }

    #[test]
    fn javascript_package_managers_refuse_only_global_installs() {
        refuses("npm", "install -g prettier", "pkg install npm:prettier");
        refuses(
            "npm",
            "i --global prettier@3 typescript",
            "pkg install npm:prettier@3 npm:typescript",
        );
        refuses("npm", "-g add @biomejs/biome", "pkg install npm:@biomejs/biome");
        refuses("npm", "uninstall -g prettier", "pkg remove npm:prettier");
        refuses("pnpm", "add -g prettier", "pkg install npm:prettier");
        refuses("pnpm", "remove --global prettier", "pkg remove npm:prettier");
        refuses("bun", "add -g prettier", "pkg install npm:prettier");
        refuses("yarn", "global add prettier", "pkg install npm:prettier");
        refuses("yarn", "global remove prettier", "pkg remove npm:prettier");
        passes("npm", "install prettier");
        passes("npm", "install");
        passes("npm", "install -g");
        passes("npm", "install -g ./local-package");
        passes("npm", "install -g prettier.tgz");
        passes("npm", "install -g https://example.com/x.tgz");
        passes("npm", "install -g --registry https://registry.example.com prettier");
        passes("npm", "install -g --prefix /opt/tools prettier");
        passes("npm", "run build");
        passes("pnpm", "add prettier");
        passes("yarn", "add prettier");
        passes("yarn", "install");
    }

    #[test]
    fn a_value_written_with_an_equals_sign_is_not_a_separate_value() {
        refuses(
            "npm",
            "install -g --registry=https://registry.example.com prettier",
            "pkg install npm:prettier",
        );
    }

    #[test]
    fn cargo_installs_crates_but_not_a_checkout_or_a_chosen_root() {
        refuses("cargo", "install ripgrep", "pkg install cargo:ripgrep");
        refuses("cargo", "install --locked cargo-watch", "pkg install cargo:cargo-watch");
        refuses("cargo", "uninstall ripgrep", "pkg remove cargo:ripgrep");
        passes("cargo", "install --path .");
        passes("cargo", "install --git https://github.com/a/b");
        passes("cargo", "install --root /opt/tools ripgrep");
        passes("cargo", "install --list");
        passes("cargo", "build --release");
    }

    #[test]
    fn python_tools_refuse_global_installs() {
        refuses("pipx", "install black", "pkg install pypi:black");
        refuses("pipx", "uninstall black", "pkg remove pypi:black");
        refuses("uv", "tool install ruff", "pkg install pypi:ruff");
        refuses("uv", "tool uninstall ruff", "pkg remove pypi:ruff");
        refuses("pip", "install requests==2.31.0", "pkg install pypi:requests@2.31.0");
        refuses("pip3", "install --user black", "pkg install pypi:black");
        passes("uv", "pip install requests");
        passes("uv", "add requests");
        passes("pip", "install -r requirements.txt");
        passes("pip", "install -e .");
        passes("pip", "install --target vendor requests");
        passes("pip", "install --target=vendor requests");
        passes("pip", "install --index-url https://pypi.example.com/simple requests");
        passes("pip", "uninstall requests");
    }

    #[test]
    fn pip_leaves_a_virtual_environment_alone() {
        assert_eq!(suggestion_in("pip", "install requests", true), None);
        assert_eq!(
            suggestion_in("pip", "install requests", false).as_deref(),
            Some("pkg install pypi:requests")
        );
    }

    #[test]
    fn go_ruby_and_dotnet_refuse_global_tool_installs() {
        refuses(
            "go",
            "install golang.org/x/tools/gopls@latest",
            "pkg install go:golang.org/x/tools/gopls",
        );
        refuses(
            "go",
            "install -v example.com/cmd/tool@v1.2.3",
            "pkg install go:example.com/cmd/tool@v1.2.3",
        );
        passes("go", "install ./...");
        passes("go", "install example.com/cmd/tool");
        passes("go", "build ./...");
        refuses("gem", "install rubocop", "pkg install gem:rubocop");
        refuses("gem", "uninstall rubocop", "pkg remove gem:rubocop");
        passes("gem", "install --install-dir vendor rubocop");
        refuses("dotnet", "tool install -g dotnet-ef", "pkg install dotnet:dotnet-ef");
        refuses("dotnet", "tool uninstall --global dotnet-ef", "pkg remove dotnet:dotnet-ef");
        passes("dotnet", "tool install dotnet-ef");
        passes("dotnet", "tool install -g dotnet-ef --tool-path /opt/tools");
        passes("dotnet", "build");
    }

    #[test]
    fn mise_refuses_global_declarations_and_names_what_was_typed() {
        refuses("mise", "use -g node@24 npm:prettier", "pkg install node@24 npm:prettier");
        refuses("mise", "use --global ripgrep", "pkg install ripgrep");
        refuses("mise", "unuse -g ripgrep", "pkg remove ripgrep");
        passes("mise", "use node@24");
        passes("mise", "install node@24");
        passes("mise", "bootstrap packages apply");
        passes("mise", "run build");
    }

    #[test]
    fn a_name_must_start_like_a_package() {
        passes("npm", "install -g -x");
        passes("brew", "install .");
        passes("brew", "install ~/tool");
        passes("brew", "install /usr/local/tool.rb");
        passes("choco", r"install C:\tools\x.nupkg.zip");
        passes("choco", "install C:/tools/x");
    }

    #[test]
    fn a_tool_with_no_rules_passes() {
        passes("ls", "-l");
    }
}
