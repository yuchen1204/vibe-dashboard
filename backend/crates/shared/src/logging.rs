use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init(level: &str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().json())
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_does_not_panic_with_valid_level() {
        init("info");
        init("debug");
    }
}
