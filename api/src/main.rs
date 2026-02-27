use api::{ApiSettings, Application};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive(tracing_subscriber::filter::LevelFilter::INFO.into()),
        )
        .init();

    let settings = ApiSettings::get_configuration().expect("Unable to read configuration files");

    let app = Application::build(settings).await?;

    app.run_until_stopped().await?;

    Ok(())
}
