use bollard::Docker;
use lapin::{Connection, ConnectionProperties};
use sqlx::PgPool;
use worker::{
    executer::{TestcaseHandler, execute},
    settings::WorkerSettings,
};

struct DefaultTestcaseHandler {}
impl TestcaseHandler for DefaultTestcaseHandler {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = WorkerSettings::get_configuration().expect("Failed to load configuration");
    let conn =
        Connection::connect(&settings.rabbitmq.url(), ConnectionProperties::default()).await?;

    let pgpool = PgPool::connect_lazy(&settings.database.url()).unwrap();

    let docker = Docker::connect_with_local_defaults()?;

    execute::<DefaultTestcaseHandler>(settings.runtimeconfigs, conn, pgpool, docker).await;
    Ok(())
}
