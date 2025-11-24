use std::path::Path;

use bollard::{Docker, secret::ContainerCreateBody};
use config::Environment;
use config_loader::get_configuration;
use deadpool::managed::Object;
use futures::future::join_all;
use futures_util::StreamExt;
use lapin::{
    Channel, Connection, ConnectionProperties, Consumer, message::Delivery, options::*,
    types::FieldTable,
};
use models::{RuntimeConfigs, WorkerTask};
use sqlx::{PgPool, types::uuid};
use thiserror::Error;
use tokio::task::JoinHandle;
use worker::{ContainerGroup, Pool, run_exec};

fn are_equal_ignore_whitespace(s1: &str, s2: &str) -> bool {
    let s1_filtered: String = s1.chars().filter(|c| !c.is_whitespace()).collect();
    let s2_filtered: String = s2.chars().filter(|c| !c.is_whitespace()).collect();
    s1_filtered == s2_filtered
}

async fn declare_queue_exchange(
    channel: &Channel,
    queue: &str,
    exchange: &str,
) -> Result<(), lapin::Error> {
    channel
        .queue_declare(queue, QueueDeclareOptions::default(), FieldTable::default())
        .await?;

    channel
        .exchange_declare(
            exchange,
            lapin::ExchangeKind::Direct,
            ExchangeDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    channel
        .queue_bind(
            queue,
            exchange,
            "",
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    Ok(())
}
#[derive(Error, Debug)]
pub enum ExecError {
    #[error("Docker error :{0}")]
    DockerError(
        #[from]
        #[source]
        bollard::errors::Error,
    ),

    #[error("RabbitMQ error :{0}")]
    QueueError(
        #[from]
        #[source]
        lapin::Error,
    ),

    #[error("Error parsing rabbitmq message")]
    ParseError,
    #[error("")]
    PoolError(#[from] deadpool::managed::PoolError<bollard::errors::Error>),
}

async fn exec_in_docker(
    docker_task: Docker,
    conn : Object<ContainerGroup>,
    code: String,
    testcase: String,
    command: String,
) -> Result<String, ExecError> {

    let mut cmd: Vec<_> = command.split(' ').map(|x| x.to_string()).collect();
    cmd.push(code);

    Ok(run_exec(&docker_task, &conn.id, cmd, testcase).await?)
}

async fn get_consumer(
    queue: &str,
    exchange: &str,
    channel: Channel,
) -> Result<Consumer, lapin::Error> {
    declare_queue_exchange(&channel, queue, exchange).await?;

    channel
        .basic_consume(
            queue,
            exchange,
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
}

enum ExecStatus {
    Passed,
    Failed,
}
impl From<ExecStatus> for &str {
    fn from(value: ExecStatus) -> Self {
         match value {
            ExecStatus::Passed => "PASSED",
            ExecStatus::Failed => "FAILED",
        }

    }
}

async fn service_request(
    docker: Docker,
    pool: Pool,
    pgpool: PgPool,
    mut consumer: Consumer,
    command: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut handles = vec![];
        while let Some(delivery) = consumer.next().await {
            let delivery = delivery.unwrap();
            let docker_task = docker.clone();
            let command = command.clone();
            let pgpool = pgpool.clone();
            
            let conn = pool.get().await.unwrap();
            let task: WorkerTask = serde_json::from_slice(&delivery.data)
                .map_err(|_| ExecError::ParseError)
                .unwrap();

            handles.push(tokio::spawn(async move {
                let row = sqlx::query!(
                    "SELECT testcase,output from problem_testcases WHERE problem_id=$1",
                    task.problem_id as i64
                )
                .fetch_one(&pgpool)
                .await
                .unwrap();

                if let Ok(exec_output) =
                    exec_in_docker(docker_task, conn, task.code, row.testcase, command).await
                {
                    let status : &str = if are_equal_ignore_whitespace(&exec_output, &row.output) {
                        ExecStatus::Passed
                    } else {
                        ExecStatus::Failed
                    }.into();
                    sqlx::query!("INSERT INTO submit_status (user_id, problem_id, output, status) VALUES($1,$2,$3,$4)",
                    task.user_id as i64, task.problem_id as i64, exec_output, status).execute(&pgpool).await.unwrap();
 
                    let _ = delivery.ack(BasicAckOptions::default()).await;
                }
            }));
        }
        join_all(handles).await;
    })
}

async fn consume(
    docker: Docker,
    pool: Pool,
    pgpool: PgPool,
    image: &str,
    channel: Channel,
    command: String,
) -> Result<JoinHandle<()>, lapin::Error> {
    let consumer = get_consumer(image, image, channel).await?;
    let docker_per_task = docker.clone();
    Ok(service_request(docker_per_task, pool, pgpool, consumer, command).await)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;
    let runtimeconfigs: RuntimeConfigs = get_configuration(Path::new("../configuration/")).unwrap();

    let pgpool = PgPool::connect("postgres://api:123@localhost:5432/judge?sslmode=disable")
        .await
        .unwrap();

    let docker = Docker::connect_with_local_defaults()?;

    let mut handles = vec![];
    for runtime in runtimeconfigs.runtimeconfigs {
        let docker_clone = docker.clone();
        let channel = conn.create_channel().await.unwrap();
        let pgpool_clone = pgpool.clone();
        let handle = tokio::spawn(async move {
            let manager = ContainerGroup::new(docker_clone.clone(), &runtime.image)
                .await
                .unwrap();
            let rabbitmq_pool = Pool::builder(manager).max_size(3).build().unwrap();

            tokio::select! {
              _ = consume(docker_clone, rabbitmq_pool.clone(), pgpool_clone.clone(), &runtime.image, channel, runtime.command).await.unwrap() => {},
             _ = tokio::signal::ctrl_c() => {
                  rabbitmq_pool.manager().close().await;
              }
            }
        });
        handles.push(handle);
    }
    join_all(handles).await;
    Ok(())
}
