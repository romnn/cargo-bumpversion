use color_eyre::eyre;
use termcolor::ColorChoice;
use tracing_subscriber::layer::SubscriberExt;

/// Setup logging
///
/// # Errors
/// - If the logging directive cannot be parsed.
/// - If the global tracing subscriber cannot be installed.
pub fn setup(
    log_level: Option<tracing::metadata::Level>,
    color_choice: ColorChoice,
) -> eyre::Result<bool> {
    let default_log_level = log_level.unwrap_or(tracing::metadata::Level::WARN);
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
    let use_color = match color_choice {
        ColorChoice::Always | ColorChoice::AlwaysAnsi => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => {
            use std::io::IsTerminal;
            std::io::stdout().is_terminal()
        }
    };

    let fmt_layer_pretty_compact = tracing_subscriber::fmt::Layer::new()
        .compact()
        .without_time()
        .with_ansi(use_color)
        .with_writer(std::io::stdout);

    let subscriber = tracing_subscriber::registry()
        .with(fmt_layer_pretty_compact)
        .with(env_filter);
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(use_color)
}
