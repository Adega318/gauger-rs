use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cli {
    Tui,
    NoTui,
    Help,
    Version,
}

impl Cli {
    pub fn parse<I>(args: I) -> Result<Self, CliError>
    where
        I: IntoIterator<Item = String>,
    {
        let mut command = Self::Tui;

        for arg in args {
            match arg.as_str() {
                "--no-tui" => command = Self::NoTui,
                "-h" | "--help" => return Ok(Self::Help),
                "-V" | "--version" => return Ok(Self::Version),
                other => return Err(CliError::UnknownArgument(other.to_owned())),
            }
        }

        Ok(command)
    }

    pub const fn usage() -> &'static str {
        concat!(
            "Gauger - monitor Docker and Podman containers\n",
            "\n",
            "Usage: gauger [OPTIONS]\n",
            "\n",
            "Options:\n",
            "      --no-tui       Stream container changes as plain text instead of starting the TUI\n",
            "  -h, --help         Print help\n",
            "  -V, --version      Print version\n",
        )
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CliError {
    #[error("unknown argument `{0}`")]
    UnknownArgument(String),
}

#[cfg(test)]
mod tests {
    use super::{Cli, CliError};

    fn parse(args: &[&str]) -> Result<Cli, CliError> {
        Cli::parse(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn defaults_to_the_tui() {
        assert_eq!(parse(&[]), Ok(Cli::Tui));
    }

    #[test]
    fn enables_plain_text_mode() {
        assert_eq!(parse(&["--no-tui"]), Ok(Cli::NoTui));
    }

    #[test]
    fn accepts_repeated_no_tui_flags() {
        assert_eq!(parse(&["--no-tui", "--no-tui"]), Ok(Cli::NoTui));
    }

    #[test]
    fn help_and_version_take_precedence() {
        assert_eq!(parse(&["--no-tui", "-h"]), Ok(Cli::Help));
        assert_eq!(parse(&["--no-tui", "--help"]), Ok(Cli::Help));
        assert_eq!(parse(&["--no-tui", "-V"]), Ok(Cli::Version));
        assert_eq!(parse(&["--no-tui", "--version"]), Ok(Cli::Version));
    }

    #[test]
    fn rejects_unknown_arguments() {
        assert_eq!(
            parse(&["--json"]),
            Err(CliError::UnknownArgument("--json".to_owned()))
        );

        assert_eq!(
            parse(&["extra"]),
            Err(CliError::UnknownArgument("extra".to_owned()))
        );
    }
}
