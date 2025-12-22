use std::path::Path;

use bollard::Docker;
use config_loader::get_configuration;
use lapin::{Connection, ConnectionProperties};
use models::{DatabaseConfig, RabbitMQConfig, RuntimeConfigs};
use sqlx::PgPool;
use worker::executer::{TestcaseHandler, execute};

struct DefaultTestcaseHandler {}
impl TestcaseHandler for DefaultTestcaseHandler {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let rabbitmq_settings: RabbitMQConfig =
        get_configuration(Path::new("../configuration/")).expect("unable to load rabbitmq");
    let postgres_settings: DatabaseConfig =
        get_configuration(Path::new("../configuration/")).unwrap();

    let conn =
        Connection::connect(&rabbitmq_settings.url(), ConnectionProperties::default()).await?;

    let runtimeconfigs: RuntimeConfigs = get_configuration(Path::new("../configuration/")).unwrap();

    let pgpool = PgPool::connect_lazy(&postgres_settings.url()).unwrap();

    let docker = Docker::connect_with_local_defaults()?;

    execute::<DefaultTestcaseHandler>(runtimeconfigs, conn, pgpool, docker).await;
    Ok(())
}
