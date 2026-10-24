//! Nexus Agent Binary - Entry point for the worker daemon

use nexus_agent::{config::AgentConfig, NexusAgent};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "nexus_agent=info,nexus_executor=info,nexus_p2p=info".into()))
        .with(tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true))
        .init();

    // Parse configuration
    let config = load_config().await?;

    // Start Prometheus metrics exporter
    let _metrics_guard = start_metrics_exporter(config.metrics_port).await?;

    // Create and run agent
    let agent = NexusAgent::new(config).await?;
    agent.run().await?;

    Ok(())
}

async fn load_config() -> anyhow::Result<AgentConfig> {
    use figment::{Figment, providers::{Env, Format, Toml, Yaml, Json}};
    use std::env;

    let config_path = env::var("NEXUS_CONFIG")
        .unwrap_or_else(|_| "./config.toml".to_string());

    let figment = Figment::new()
        .merge(Toml::file(&config_path).nested())
        .merge(Yaml::file(&config_path).nested())
        .merge(Json::file(&config_path).nested())
        .merge(Env::prefixed("NEXUS_").global())
        .merge(Env::prefixed("NEXUS_").split("__"));

    figment.extract().map_err(Into::into)
}

async fn start_metrics_exporter(port: u16) -> anyhow::Result<metrics_exporter_prometheus::PrometheusHandle> {
    use metrics_exporter_prometheus::PrometheusBuilder;

    let builder = PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], port).into())
        .build()?;

    println!("Metrics exporter listening on http://0.0.0.0:{}/metrics", port);

    Ok(builder)
}