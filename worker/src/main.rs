use bollard::Docker;
use lapin::{Connection, ConnectionProperties};
use sqlx::PgPool;
use worker::{
    executer::{TestcaseHandler, execute},
    settings::WorkerSettings,
};
use tracing_subscriber::EnvFilter;

struct DefaultTestcaseHandler {}
impl TestcaseHandler for DefaultTestcaseHandler {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive(tracing_subscriber::filter::LevelFilter::INFO.into()),
        )
        .init();

    let settings = WorkerSettings::get_configuration().expect("Failed to load configuration");
    let conn =
        Connection::connect(&settings.rabbitmq.url(), ConnectionProperties::default()).await?;

    let pgpool = PgPool::connect_lazy(&settings.database.url()).unwrap();

    let docker = Docker::connect_with_local_defaults()?;

    execute::<DefaultTestcaseHandler>(settings.runtimeconfig, conn, pgpool, docker).await;
    Ok(())
}
