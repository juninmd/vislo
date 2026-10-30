//! Command-line actions: `--help`, `--version` and file-association registration. Parsing and the
//! texts are pure, so they are tested without a console or the registry.

use crate::assoc;
use std::io::Write;

/// What the command line asks for; `Gui` means start the viewer as usual.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Gui,
    Help,
    Version,
    Register(Vec<String>),
    Unregister,
    /// Bad usage: the message for stderr, exit code 2.
    Usage(String),
}

const HINT: &str = "Try \"vislo --help\" for usage.";

/// Accepted anywhere on the command line; any other `--flag` is a usage error.
const KNOWN_FLAGS: &[&str] = &[
    "--background",
    "--help",
    "--version",
    "--register-associations",
    "--unregister-associations",
];

/// Parses the arguments of the process (the first one is the executable name).
pub fn parse(args: impl IntoIterator<Item = String>) -> Action {
    let args: Vec<String> = args.into_iter().skip(1).collect();
    if let Some(flag) = args
        .iter()
        .find(|a| a.starts_with("--") && !KNOWN_FLAGS.contains(&a.as_str()))
    {
        return Action::Usage(format!("unknown option \"{flag}\"\n{HINT}"));
    }
    match args.first().map(String::as_str) {
        Some("--help" | "-h") => Action::Help,
        Some("--version" | "-V") => Action::Version,
        Some("--unregister-associations") => Action::Unregister,
        Some("--register-associations") => {
            // Optional comma-separated format groups, e.g. `jpeg,png,raw`; none means all.
            let groups: Vec<String> = args
                .get(1)
                .map(|g| g.split(',').map(str::to_string).collect())
                .unwrap_or_default();
            match assoc::extensions_for(&groups) {
                Ok(_) => Action::Register(groups),
                Err(e) => Action::Usage(format!("{e}\nvalid groups: {}\n{HINT}", group_names())),
            }
        }
        _ => Action::Gui,
    }
}

fn group_names() -> String {
    assoc::GROUPS
        .iter()
        .map(|(g, _)| *g)
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn help_text() -> String {
    let lines = [
        "Vislo: fast image viewer.",
        "",
        "Usage:",
        "  vislo [FILE]                            open FILE; its folder becomes the album",
        "  vislo --background                      start parked in the tray (needs Keep ready)",
        "  vislo --register-associations [GROUPS]  make Vislo the default for GROUPS",
        "  vislo --unregister-associations         remove that registration",
        "  vislo --help, -h                        show this help",
        "  vislo --version, -V                     show the version",
    ];
    format!(
        "{}\n\nGroups (comma-separated, default all): {}\n\n\
         Exit codes: 0 success, 1 failure, 2 bad usage.",
        lines.join("\n"),
        group_names()
    )
}

pub fn version_text() -> String {
    format!("vislo {}", env!("CARGO_PKG_VERSION"))
}

/// Runs a headless action and returns its exit code; `None` when the viewer should start.
pub fn run() -> Option<i32> {
    let action = parse(std::env::args());
    if action == Action::Gui {
        return None;
    }
    attach_parent_console();
    let result = match action {
        Action::Usage(message) => {
            let _ = writeln!(std::io::stderr(), "vislo: {message}");
            return Some(2);
        }
        Action::Help => {
            print(&help_text());
            Ok(())
        }
        Action::Version => {
            print(&version_text());
            Ok(())
        }
        Action::Register(groups) => assoc::register(&groups).and_then(|()| assoc::open_settings()),
        Action::Unregister => assoc::unregister(),
        Action::Gui => Ok(()),
    };
    Some(match result {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "vislo: {e}");
            1
        }
    })
}

fn print(text: &str) {
    // A closed pipe (`vislo --help | head`) is not an error worth reporting.
    let _ = writeln!(std::io::stdout(), "{text}");
}

/// The release exe is a GUI-subsystem binary with no console: attach the one that started it so
/// the output reaches the terminal. Best effort: from Explorer there is no parent console.
#[cfg(windows)]
fn attach_parent_console() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    // SAFETY: plain Win32 call with no pointers; failure just leaves the output unattached.
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

#[cfg(not(windows))]
fn attach_parent_console() {}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli(args: &[&str]) -> Action {
        parse(std::iter::once("vislo.exe".to_string()).chain(args.iter().map(|a| a.to_string())))
    }

    #[test]
    fn plain_launches_and_background_start_the_viewer() {
        assert_eq!(cli(&[]), Action::Gui);
        assert_eq!(cli(&["--background"]), Action::Gui);
        assert_eq!(cli(&[r"C:\pics\a.jpg"]), Action::Gui);
        assert_eq!(cli(&["--background", r"C:\pics\a.jpg"]), Action::Gui);
    }

    #[test]
    fn the_first_argument_is_the_executable_not_an_action() {
        let args = ["--help"].map(String::from);
        assert_eq!(parse(args), Action::Gui);
    }

    #[test]
    fn help_and_version_have_long_and_short_forms() {
        assert_eq!(cli(&["--help"]), Action::Help);
        assert_eq!(cli(&["-h"]), Action::Help);
        assert_eq!(cli(&["--version"]), Action::Version);
        assert_eq!(cli(&["-V"]), Action::Version);
    }

    #[test]
    fn unregister_takes_no_argument() {
        assert_eq!(cli(&["--unregister-associations"]), Action::Unregister);
    }

    #[test]
    fn register_without_groups_means_all_of_them() {
        assert_eq!(cli(&["--register-associations"]), Action::Register(vec![]));
    }

    #[test]
    fn register_accepts_a_comma_separated_group_list() {
        assert_eq!(
            cli(&["--register-associations", "jpeg,png,raw"]),
            Action::Register(vec!["jpeg".into(), "png".into(), "raw".into()])
        );
    }

    #[test]
    fn an_unknown_group_is_a_usage_error_that_lists_the_valid_ones() {
        let Action::Usage(msg) = cli(&["--register-associations", "nope"]) else {
            panic!("expected a usage error");
        };
        assert!(msg.starts_with("unknown format group \"nope\""), "{msg}");
        for (group, _) in assoc::GROUPS {
            assert!(msg.contains(group), "missing {group} in {msg}");
        }
    }

    #[test]
    fn an_empty_group_in_the_list_is_rejected_not_ignored() {
        assert!(matches!(
            cli(&["--register-associations", "jpeg,,png"]),
            Action::Usage(_)
        ));
        assert!(matches!(
            cli(&["--register-associations", ""]),
            Action::Usage(_)
        ));
    }

    #[test]
    fn a_misspelled_flag_is_a_usage_error_even_next_to_a_file() {
        let Action::Usage(msg) = cli(&["--register-associatons"]) else {
            panic!("expected a usage error");
        };
        assert!(
            msg.starts_with("unknown option \"--register-associatons\""),
            "{msg}"
        );
        assert!(matches!(
            cli(&[r"C:\pics\a.jpg", "--bogus"]),
            Action::Usage(_)
        ));
    }

    #[test]
    fn single_dash_arguments_other_than_the_short_flags_are_left_alone() {
        assert_eq!(cli(&["-x"]), Action::Gui);
    }

    #[test]
    fn help_lists_every_action_group_and_exit_code() {
        let help = help_text();
        for flag in [
            "--background",
            "--register-associations",
            "--version",
            "--help",
        ] {
            assert!(help.contains(flag), "missing {flag}");
        }
        for (group, _) in assoc::GROUPS {
            assert!(help.contains(group), "missing {group}");
        }
        assert!(help.contains("2 bad usage"));
    }

    #[test]
    fn version_is_the_package_version() {
        assert_eq!(
            version_text(),
            format!("vislo {}", env!("CARGO_PKG_VERSION"))
        );
    }
}
