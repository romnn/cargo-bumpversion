use color_eyre::eyre;
use termcolor::ColorChoice;
use tracing_subscriber::layer::SubscriberExt;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogFormat {
    Json,
    PrettyCompact,
    Pretty,
}

impl std::str::FromStr for LogFormat {
    type Err = eyre::Report;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            s if s.eq_ignore_ascii_case("json") => Ok(LogFormat::Json),
            s if s.eq_ignore_ascii_case("pretty") => Ok(LogFormat::Pretty),
            s if s.eq_ignore_ascii_case("pretty-compact") => Ok(LogFormat::PrettyCompact),
            other => Err(eyre::eyre!("unknown log format: {other:?}")),
        }
    }
}

/// Setup logging
///
/// # Errors
/// - If the logging directive cannot be parsed.
/// - If the global tracing subscriber cannot be installed.
pub fn setup(
    log_level: Option<tracing::metadata::Level>,
    log_format: Option<LogFormat>,
    color_choice: ColorChoice,
) -> eyre::Result<(LogFormat, bool)> {
    let default_log_level = log_level.unwrap_or(tracing::metadata::Level::INFO);
    let default_log_directive = format!(
        "none,bumpversion={}",
        default_log_level.to_string().to_ascii_lowercase()
    );
    let default_env_filter = tracing_subscriber::filter::EnvFilter::builder()
        .with_regex(true)
        .with_default_directive(default_log_level.into())
        .parse(default_log_directive)?;

    let env_filter_directive = std::env::var("RUST_LOG").ok();
    let env_filter = match env_filter_directive {
        Some(directive) => {
            match tracing_subscriber::filter::EnvFilter::builder()
                .with_env_var(directive)
                .try_from_env()
            {
                Ok(env_filter) => env_filter,
                Err(err) => {
                    eprintln!("invalid log filter: {err}");
                    eprintln!("falling back to default logging");
                    default_env_filter
                }
            }
        }
        None => default_env_filter,
    };

    // autodetect logging format
    let log_format = log_format.unwrap_or(LogFormat::PrettyCompact);
    let use_color = match color_choice {
        ColorChoice::Always | ColorChoice::AlwaysAnsi => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => {
            use std::io::IsTerminal;
            std::io::stdout().is_terminal()
        }
    };

    let fmt_layer_pretty = tracing_subscriber::fmt::Layer::new()
        .pretty()
        .without_time()
        .with_ansi(use_color)
        .fmt_fields(tracing_subscriber::fmt::format::PrettyFields::new())
        .with_writer(std::io::stdout);
    let fmt_layer_pretty_compact = tracing_subscriber::fmt::Layer::new()
        .compact()
        .without_time()
        .with_ansi(use_color)
        .with_writer(std::io::stdout);
    let fmt_layer_json = tracing_subscriber::fmt::Layer::new()
        .json()
        .compact()
        .without_time()
        .with_ansi(use_color)
        .with_writer(std::io::stdout);

    let subscriber = tracing_subscriber::registry()
        .with(if log_format == LogFormat::Json {
            Some(fmt_layer_json)
        } else {
            None
        })
        .with(if log_format == LogFormat::PrettyCompact {
            Some(fmt_layer_pretty_compact)
        } else {
            None
        })
        .with(if log_format == LogFormat::Pretty {
            Some(fmt_layer_pretty)
        } else {
            None
        })
        .with(env_filter);
    tracing::subscriber::set_global_default(subscriber)?;
    Ok((log_format, use_color))
}
