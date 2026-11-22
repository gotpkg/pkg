//! What each shim intercepts: the table behind `docs/packages.md#what-a-shim-refuses`.

/// An operating system, as far as which package managers mise runs on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum System {
    Macos,
    Linux,
    Windows,
}

impl System {
    pub(crate) const fn current() -> System {
        if cfg!(target_os = "macos") {
            System::Macos
        } else if cfg!(windows) {
            System::Windows
        } else {
            System::Linux
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Install,
    Remove,
}

impl Action {
    /// The `pkg` command that does the same job.
    pub(crate) fn pkg_command(self) -> &'static str {
        match self {
            Action::Install => "install",
            Action::Remove => "remove",
        }
    }
}

/// One package manager's shim.
#[derive(Clone, Copy)]
pub(crate) struct Tool {
    pub(crate) name: &'static str,
    /// The systems the shim is installed on.
    pub(crate) systems: &'static [System],
    /// Options the tool documents as taking a separate value. The value cannot be told from a
    /// package name, so an invocation that uses one is not intercepted.
    pub(crate) value_options: &'static [&'static str],
    /// Options whose value is the package itself, such as `winget install --id`.
    pub(crate) name_options: &'static [&'static str],
    pub(crate) rules: &'static [Rule],
}

/// One kind of invocation a shim refuses.
#[derive(Clone, Copy)]
pub(crate) struct Rule {
    pub(crate) action: Action,
    pub(crate) form: Form,
    pub(crate) names: Names,
    /// At least one of these must be present; none required when empty.
    pub(crate) requires_any: &'static [&'static str],
    /// Any of these means the package goes where the person chose, or comes from elsewhere.
    pub(crate) unless_any: &'static [&'static str],
    /// Python's `pip` installs into the active virtual environment, which is a project's.
    pub(crate) outside_virtual_environment: bool,
    pub(crate) target: Target,
}

/// How the verb is written.
#[derive(Clone, Copy)]
pub(crate) enum Form {
    /// `<tool> <prefix...> <verb> <names...>`.
    Verb { prefix: &'static [&'static str], verbs: &'static [&'static str] },
    /// pacman's `-S <names...>`, which has no verb.
    Sync,
}

/// How the names after the verb are read.
#[derive(Clone, Copy)]
pub(crate) enum Names {
    Plain,
    /// `module@version` only: without a version `go install` builds the current module.
    Versioned,
    /// A leading remote name, which has no `.` where an application ID always has one.
    AfterRemote,
}

/// How a name becomes a package for `pkg`.
#[derive(Clone, Copy)]
pub(crate) struct Target {
    /// The prefix `pkg` takes; none for names that are already mise's own.
    pub(crate) manager: &'static str,
    /// Options that select another manager's prefix, such as `--cask`.
    pub(crate) variant: Option<(&'static [&'static str], &'static str)>,
    /// The manager's version separator, which `pkg` writes as `@`.
    pub(crate) version_separator: Option<&'static str>,
    /// Another manager whose prefix may be the right one for the same name.
    pub(crate) also: Option<&'static str>,
}

const NONE: &[&str] = &[];
const GLOBAL: &[&str] = &["-g", "--global"];

const ALL_SYSTEMS: &[System] = &[System::Macos, System::Linux, System::Windows];
const MACOS_AND_LINUX: &[System] = &[System::Macos, System::Linux];
const MACOS: &[System] = &[System::Macos];
const LINUX: &[System] = &[System::Linux];
const WINDOWS: &[System] = &[System::Windows];

impl Target {
    const fn new(manager: &'static str) -> Target {
        Target { manager, variant: None, version_separator: None, also: None }
    }

    const fn variant(self, options: &'static [&'static str], manager: &'static str) -> Target {
        Target { variant: Some((options, manager)), ..self }
    }

    const fn separator(self, separator: &'static str) -> Target {
        Target { version_separator: Some(separator), ..self }
    }

    const fn also(self, manager: &'static str) -> Target {
        Target { also: Some(manager), ..self }
    }
}

impl Rule {
    const fn new(action: Action, form: Form, target: Target) -> Rule {
        Rule {
            action,
            form,
            names: Names::Plain,
            requires_any: NONE,
            unless_any: NONE,
            outside_virtual_environment: false,
            target,
        }
    }

    const fn install(verbs: &'static [&'static str], target: Target) -> Rule {
        Rule::new(Action::Install, Form::Verb { prefix: NONE, verbs }, target)
    }

    const fn remove(verbs: &'static [&'static str], target: Target) -> Rule {
        Rule::new(Action::Remove, Form::Verb { prefix: NONE, verbs }, target)
    }

    const fn after(self, prefix: &'static [&'static str]) -> Rule {
        let Form::Verb { verbs, .. } = self.form else { return self };
        Rule { form: Form::Verb { prefix, verbs }, ..self }
    }

    const fn requiring(self, options: &'static [&'static str]) -> Rule {
        Rule { requires_any: options, ..self }
    }

    const fn unless(self, options: &'static [&'static str]) -> Rule {
        Rule { unless_any: options, ..self }
    }

    const fn names(self, names: Names) -> Rule {
        Rule { names, ..self }
    }

    const fn outside_virtual_environment(self) -> Rule {
        Rule { outside_virtual_environment: true, ..self }
    }
}

impl Tool {
    const fn new(name: &'static str, systems: &'static [System], rules: &'static [Rule]) -> Tool {
        Tool { name, systems, value_options: NONE, name_options: NONE, rules }
    }

    const fn with_values(self, options: &'static [&'static str]) -> Tool {
        Tool { value_options: options, ..self }
    }

    const fn named_by(self, options: &'static [&'static str]) -> Tool {
        Tool { name_options: options, ..self }
    }
}

const PACMAN_VALUES: &[&str] = &[
    "--config",
    "--root",
    "-r",
    "--dbpath",
    "-b",
    "--cachedir",
    "--logfile",
    "--gpgdir",
    "--hookdir",
    "--arch",
    "--color",
    "--sysroot",
    "--ignore",
    "--ignoregroup",
    "--assume-installed",
    "--overwrite",
    "--print-format",
];
const AUR_HELPER_VALUES: &[&str] = &[
    "--config",
    "--root",
    "-r",
    "--dbpath",
    "-b",
    "--cachedir",
    "--logfile",
    "--gpgdir",
    "--hookdir",
    "--arch",
    "--color",
    "--sysroot",
    "--ignore",
    "--ignoregroup",
    "--assume-installed",
    "--overwrite",
    "--print-format",
    "--builddir",
    "--editor",
    "--makepkg",
    "--pacman",
    "--sudo",
    "--git",
    "--gpg",
    "--mflags",
    "--gitflags",
    "--sudoflags",
];
const PIP_BYPASS: &[&str] =
    &["--target", "-t", "--prefix", "--root", "-r", "--requirement", "-e", "--editable"];
const MISE_VALUES: &[&str] =
    &["-C", "--cd", "-E", "--env", "-j", "--jobs", "-p", "--path", "--before"];
const PYTHON_INDEX_VALUES: &[&str] = &[
    "-i",
    "--index-url",
    "--extra-index-url",
    "-f",
    "--find-links",
    "-c",
    "--constraint",
    "--python",
    "--python-version",
    "--platform",
    "--implementation",
    "--abi",
    "--src",
    "--proxy",
    "--retries",
    "--timeout",
    "--cert",
    "--client-cert",
    "--cache-dir",
    "--log",
    "--no-binary",
    "--only-binary",
    "--upgrade-strategy",
    "--progress-bar",
    "--report",
    "--config-settings",
    "-C",
    "--global-option",
    "--install-option",
    "--use-feature",
    "--exists-action",
    "--trusted-host",
    "--target",
    "-t",
    "--prefix",
    "--root",
    "-r",
    "--requirement",
    "-e",
    "--editable",
];

/// Every shim, in the order the reference lists them.
pub(crate) static TOOLS: &[Tool] = &[
    Tool::new(
        "brew",
        MACOS_AND_LINUX,
        &[
            Rule::install(
                &["install"],
                Target::new("brew").variant(&["--cask", "--casks"], "brew-cask"),
            ),
            Rule::remove(
                &["uninstall", "remove", "rm"],
                Target::new("brew").variant(&["--cask", "--casks"], "brew-cask"),
            ),
        ],
    ),
    Tool::new("apt", LINUX, &[Rule::install(&["install"], Target::new("apt").separator("="))])
        .with_values(&[
            "-o",
            "--option",
            "-t",
            "--target-release",
            "--default-release",
            "-c",
            "--config-file",
        ]),
    Tool::new("apt-get", LINUX, &[Rule::install(&["install"], Target::new("apt").separator("="))])
        .with_values(&[
            "-o",
            "--option",
            "-t",
            "--target-release",
            "--default-release",
            "-c",
            "--config-file",
        ]),
    Tool::new("dnf", LINUX, &[Rule::install(&["install", "in"], Target::new("dnf"))]).with_values(
        &[
            "-c",
            "--config",
            "--enablerepo",
            "--disablerepo",
            "--repo",
            "--repoid",
            "--releasever",
            "--setopt",
            "--installroot",
            "-x",
            "--exclude",
            "--forcearch",
        ],
    ),
    Tool::new("zypper", LINUX, &[Rule::install(&["install", "in"], Target::new("zypper"))])
        .with_values(&["-r", "--repo", "-t", "--type", "--from", "-R", "--root"]),
    Tool::new("apk", LINUX, &[Rule::install(&["add"], Target::new("apk"))]).with_values(&[
        "-X",
        "--repository",
        "-p",
        "--root",
        "-t",
        "--virtual",
        "-C",
        "--cache-dir",
        "--keys-dir",
        "--repositories-file",
    ]),
    Tool::new("pacman", LINUX, &[Rule::new(Action::Install, Form::Sync, Target::new("pacman"))])
        .with_values(PACMAN_VALUES),
    Tool::new(
        "yay",
        LINUX,
        &[Rule::new(Action::Install, Form::Sync, Target::new("aur").also("pacman"))],
    )
    .with_values(AUR_HELPER_VALUES),
    Tool::new(
        "paru",
        LINUX,
        &[Rule::new(Action::Install, Form::Sync, Target::new("aur").also("pacman"))],
    )
    .with_values(AUR_HELPER_VALUES),
    Tool::new(
        "flatpak",
        LINUX,
        &[Rule::install(&["install"], Target::new("flatpak").variant(&["--user"], "flatpak-user"))
            .names(Names::AfterRemote)],
    )
    .with_values(&[
        "--arch",
        "--commit",
        "--installation",
        "--subpath",
        "--sideload-repo",
        "--gpg-file",
    ]),
    Tool::new("snap", LINUX, &[Rule::install(&["install"], Target::new("snap"))]).with_values(&[
        "--channel",
        "--revision",
        "--cohort",
        "--name",
    ]),
    Tool::new(
        "nix",
        MACOS_AND_LINUX,
        &[Rule::install(&["install", "add"], Target::new("nix")).after(&["profile"])],
    )
    .with_values(&[
        "--profile",
        "-I",
        "--option",
        "--override-input",
        "--inputs-from",
        "-j",
        "--max-jobs",
        "--priority",
    ]),
    Tool::new("port", MACOS, &[Rule::install(&["install"], Target::new("macports"))])
        .with_values(&["-D", "-F"]),
    Tool::new("mas", MACOS, &[Rule::install(&["install"], Target::new("mas"))]),
    Tool::new("winget", WINDOWS, &[Rule::install(&["install"], Target::new("winget"))])
        .named_by(&["--id"])
        .with_values(&[
            "-s",
            "--source",
            "-v",
            "--version",
            "--scope",
            "-a",
            "--architecture",
            "--locale",
            "-l",
            "--location",
            "-o",
            "--log",
            "--override",
            "--custom",
            "--installer-type",
            "-q",
            "--query",
            "--name",
            "-m",
            "--moniker",
            "--header",
        ]),
    Tool::new("scoop", WINDOWS, &[Rule::install(&["install"], Target::new("scoop"))])
        .with_values(&["-a", "--arch"]),
    Tool::new("choco", WINDOWS, &[Rule::install(&["install"], Target::new("choco"))]).with_values(
        &[
            "-s",
            "--source",
            "--version",
            "--ia",
            "--installarguments",
            "--install-arguments",
            "--params",
            "--packageparameters",
            "-u",
            "--user",
            "-p",
            "--password",
        ],
    ),
    Tool::new(
        "npm",
        ALL_SYSTEMS,
        &[
            Rule::install(&["install", "i", "add"], Target::new("npm")).requiring(GLOBAL),
            Rule::remove(&["uninstall", "un", "rm", "remove"], Target::new("npm"))
                .requiring(GLOBAL),
        ],
    )
    .with_values(&[
        "--registry",
        "--prefix",
        "--tag",
        "--cache",
        "--userconfig",
        "--globalconfig",
        "--loglevel",
        "-w",
        "--workspace",
        "--omit",
        "--include",
        "--otp",
        "--before",
        "--install-strategy",
        "--scope",
    ]),
    Tool::new(
        "pnpm",
        ALL_SYSTEMS,
        &[
            Rule::install(&["add", "install", "i"], Target::new("npm")).requiring(GLOBAL),
            Rule::remove(&["remove", "rm", "uninstall", "un"], Target::new("npm"))
                .requiring(GLOBAL),
        ],
    )
    .with_values(&[
        "--registry",
        "--dir",
        "-C",
        "--filter",
        "-F",
        "--store-dir",
        "--global-dir",
        "--global-bin-dir",
        "--reporter",
        "--loglevel",
    ]),
    Tool::new(
        "bun",
        ALL_SYSTEMS,
        &[
            Rule::install(&["add", "install", "i", "a"], Target::new("npm")).requiring(GLOBAL),
            Rule::remove(&["remove", "rm", "uninstall"], Target::new("npm")).requiring(GLOBAL),
        ],
    )
    .with_values(&[
        "--registry",
        "--cwd",
        "--cache-dir",
        "--config",
        "-c",
        "--concurrent-scripts",
        "--backend",
        "--network-concurrency",
    ]),
    Tool::new(
        "yarn",
        ALL_SYSTEMS,
        &[
            Rule::install(&["add"], Target::new("npm")).after(&["global"]),
            Rule::remove(&["remove"], Target::new("npm")).after(&["global"]),
        ],
    )
    .with_values(&[
        "--registry",
        "--cwd",
        "--prefix",
        "--cache-folder",
        "--modules-folder",
        "--network-timeout",
        "--proxy",
        "--https-proxy",
        "--mutex",
        "--link-folder",
        "--global-folder",
    ]),
    Tool::new(
        "cargo",
        ALL_SYSTEMS,
        &[
            Rule::install(&["install"], Target::new("cargo"))
                .unless(&["--path", "--git", "--root", "--list"]),
            Rule::remove(&["uninstall"], Target::new("cargo")).unless(&["--root"]),
        ],
    )
    .with_values(&[
        "--version",
        "--vers",
        "--registry",
        "--index",
        "--features",
        "-F",
        "--target",
        "--profile",
        "-j",
        "--jobs",
        "--bin",
        "--example",
        "--target-dir",
        "--config",
        "--color",
        "-Z",
        "--message-format",
        "--rev",
        "--tag",
        "--branch",
        "--root",
        "--path",
        "--git",
    ]),
    Tool::new(
        "pip",
        ALL_SYSTEMS,
        &[Rule::install(&["install"], Target::new("pypi").separator("=="))
            .unless(PIP_BYPASS)
            .outside_virtual_environment()],
    )
    .with_values(PYTHON_INDEX_VALUES),
    Tool::new(
        "pip3",
        ALL_SYSTEMS,
        &[Rule::install(&["install"], Target::new("pypi").separator("=="))
            .unless(PIP_BYPASS)
            .outside_virtual_environment()],
    )
    .with_values(PYTHON_INDEX_VALUES),
    Tool::new(
        "pipx",
        ALL_SYSTEMS,
        &[
            Rule::install(&["install"], Target::new("pypi")),
            Rule::remove(&["uninstall"], Target::new("pypi")),
        ],
    )
    .with_values(&[
        "--python",
        "-i",
        "--index-url",
        "--pip-args",
        "--suffix",
        "--preinstall",
        "--spec",
    ]),
    Tool::new(
        "uv",
        ALL_SYSTEMS,
        &[
            Rule::install(&["install"], Target::new("pypi")).after(&["tool"]),
            Rule::remove(&["uninstall"], Target::new("pypi")).after(&["tool"]),
        ],
    )
    .with_values(&[
        "--python",
        "-p",
        "--index",
        "--default-index",
        "-i",
        "--index-url",
        "--extra-index-url",
        "-f",
        "--find-links",
        "--with",
        "--with-requirements",
        "--from",
        "-c",
        "--constraints",
        "--overrides",
        "--config-file",
        "--directory",
        "--project",
        "--cache-dir",
        "--color",
    ]),
    Tool::new(
        "go",
        ALL_SYSTEMS,
        &[Rule::install(&["install"], Target::new("go")).names(Names::Versioned)],
    )
    .with_values(&[
        "-ldflags",
        "-gcflags",
        "-asmflags",
        "-tags",
        "-o",
        "-C",
        "-p",
        "-mod",
        "-modfile",
        "-overlay",
        "-pkgdir",
        "-toolexec",
        "-buildmode",
        "-compiler",
        "-installsuffix",
        "-gccgoflags",
        "-coverpkg",
    ]),
    Tool::new(
        "gem",
        ALL_SYSTEMS,
        &[
            Rule::install(&["install"], Target::new("gem")).unless(&["-i", "--install-dir"]),
            Rule::remove(&["uninstall"], Target::new("gem")).unless(&["-i", "--install-dir"]),
        ],
    )
    .with_values(&[
        "-i",
        "--install-dir",
        "-n",
        "--bindir",
        "-v",
        "--version",
        "-s",
        "--source",
        "--platform",
        "-P",
        "--trust-policy",
        "-p",
        "--http-proxy",
        "--config-file",
    ]),
    Tool::new(
        "dotnet",
        ALL_SYSTEMS,
        &[
            Rule::install(&["install"], Target::new("dotnet"))
                .after(&["tool"])
                .requiring(GLOBAL)
                .unless(&["--tool-path"]),
            Rule::remove(&["uninstall"], Target::new("dotnet"))
                .after(&["tool"])
                .requiring(GLOBAL)
                .unless(&["--tool-path"]),
        ],
    )
    .with_values(&[
        "--version",
        "--add-source",
        "--configfile",
        "--framework",
        "--tool-path",
        "--tool-manifest",
        "-v",
        "--verbosity",
        "-a",
        "--arch",
    ]),
    Tool::new(
        "mise",
        ALL_SYSTEMS,
        &[
            Rule::install(&["use", "u"], Target::new("")).requiring(GLOBAL),
            Rule::remove(&["unuse", "rm", "remove"], Target::new("")).requiring(GLOBAL),
        ],
    )
    .with_values(MISE_VALUES),
];

/// The shim named `name`.
pub(crate) fn find(name: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|tool| tool.name == name)
}

/// The shims installed on `system`.
pub(crate) fn for_system(system: System) -> impl Iterator<Item = &'static Tool> {
    TOOLS.iter().filter(move |tool| tool.systems.contains(&system))
}
