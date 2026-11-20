use pkg_core::{Error, ErrorKind, Recovery};

use crate::NAME;
use crate::commands::common::for_people;
use crate::environment::MISE_VARIABLE;
use crate::failure::{ErrorCode, Failure};

impl From<Error> for Failure {
    fn from(mut error: Error) -> Self {
        let code = match error.kind {
            ErrorKind::InvalidArgument => ErrorCode::InvalidArgument,
            ErrorKind::NotFound => ErrorCode::NotFound,
            ErrorKind::PreconditionFailed => ErrorCode::PreconditionFailed,
            ErrorKind::Unavailable => ErrorCode::Unavailable,
            ErrorKind::MiseFailed => ErrorCode::MiseFailed,
            ErrorKind::Internal => ErrorCode::Internal,
        };
        if error.details.program.is_some() {
            error.details.variable = Some(MISE_VARIABLE.to_string());
        }
        if let (Some(package), Some(config), Some(editable)) =
            (&error.details.package, &error.details.config, &error.details.editable_config)
        {
            error.message = format!(
                "{package} is declared in {}, and pkg only edits {}",
                for_people(config),
                for_people(editable)
            );
        }
        for recovery in &error.recovery {
            if let Recovery::RemoveTap { tap, config, editable_config } = recovery {
                error.message = format!(
                    "{tap} is declared in {}, and pkg only edits {}",
                    for_people(config),
                    for_people(editable_config)
                );
            }
        }
        let details = crate::error_details::render(&error.details);
        Failure {
            code,
            message: error.message,
            details: Box::new(details),
            suggestions: error.recovery.into_iter().map(hint).collect(),
        }
    }
}

fn hint(recovery: Recovery) -> String {
    match recovery {
        Recovery::InstallMise => format!("Install mise: {NAME} setup"),
        Recovery::SelectMise => format!("Or point {MISE_VARIABLE} at the mise executable"),
        Recovery::CheckMiseVersion => {
            "Check that mise is 2026.11.9 or newer: mise --version".into()
        }
        Recovery::RetryNetwork => "Check the network connection and retry the same command".into(),
        Recovery::ListPackages => format!("{NAME} list"),
        Recovery::Search(query) => format!("{NAME} search {query}"),
        Recovery::Info(package) => format!("{NAME} info {package}"),
        Recovery::Install(package) => format!("{NAME} install {package}"),
        Recovery::PreviewPrune => format!("See what a prune removes: {NAME} prune --dry-run"),
        Recovery::Prune => format!("Uninstall them first: {NAME} prune"),
        Recovery::Keep(package) => format!("Or keep {package}: {NAME} install {package}"),
        Recovery::RemoveDeclaration { package, config, shared } => {
            if shared {
                format!(
                    "Remove `{package}` from {} by hand; it may be shared with other machines",
                    for_people(config)
                )
            } else {
                format!("Delete the `\"{package}\"` entry from {} by hand", config.display())
            }
        }
        Recovery::ListTaps => format!("{NAME} tap list"),
        Recovery::RemoveTap { tap, config, .. } => {
            format!("Remove `{tap}` from {} by hand", for_people(config))
        }
        Recovery::Cursor => {
            "Pass the nextCursor value from the previous page, or omit --cursor".into()
        }
        Recovery::UpdateMise => "Update mise: mise self-update".into(),
        Recovery::UpdateMiseWithItsPackageManager => {
            "Update mise with the package manager that installed it".into()
        }
        Recovery::MoveAside(path) => {
            format!("Move {} aside, then run the same command again", for_people(path))
        }
        Recovery::RunInTerminal => {
            "Run the same command in a terminal, where mise can ask for what it needs".into()
        }
        Recovery::RunAgain => {
            "Fix the cause and run the same command again; parts already done are left alone".into()
        }
    }
}
