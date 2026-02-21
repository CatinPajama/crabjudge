use crate::docker::ExecOutput;
use crate::error::ExecError;
use crate::pool::ContainerGroup;
use bollard::Docker;
use deadpool::managed::{self, Object, Pool};
use futures::future::join_all;
use futures_util::StreamExt;
use lapin::{Channel, Consumer, options::*, types::FieldTable};
use models::{ExecStatus, RuntimeConfig, RuntimeConfigs, WorkerTask};
use sqlx::PgPool;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::signal::unix::{SignalKind, signal};
use tokio_util::sync::CancellationToken;

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
            queue,
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    Ok(())
}

pub async fn exec_testcase(
    docker_task: Docker,
    container_id: &str,
    code: &str,
    testcase: &str,
    compile: &Option<String>,
    run: &str,
    timeout: u8,
) -> Result<ExecOutput, ExecError> {
    let command = if let Some(compile) = compile {
        format!("{} && timeout {timeout}s {}", compile, run)
    } else {
        format!("timeout {timeout}s {}", run)
    };
    let cmd = vec![
        "sh".into(),
        "-c".into(),
        format!("printf '%s' \"$1\" > /tmp/file && {}", command),
        "--".into(),
        code.into(),
    ];
    println!("{:?}", cmd);

    Ok(crate::docker::run_exec(&docker_task, container_id, cmd, testcase).await?)
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
            "",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
}

async fn listen<T: TestcaseHandler>(
    docker: &Docker,
    pool: &managed::Pool<ContainerGroup>,
    pgpool: PgPool,
    mut consumer: Consumer,
    compile: Option<String>,
    run: String,
    timeout: u8,
    token: CancellationToken,
) -> Result<(), ExecError> {
    let mut handles = vec![];
    while let Some(delivery) = consumer.next().await {
        let delivery = delivery?;
        let docker_task = docker.clone();
        let run = run.clone();
        let compile = compile.clone();
        let pgpool = pgpool.clone();

        let conn: Object<ContainerGroup> = pool.get().await?;
        let task: WorkerTask =
            serde_json::from_slice(&delivery.data).map_err(|_| ExecError::ParseError)?;
        let token = token.clone();
        handles.push(tokio::spawn(async move {
            tokio::select! {
                _ = token.cancelled() => {
                    println!("closing");
                    delivery
                            .nack(BasicNackOptions {
                                multiple: true,
                                requeue: true,
                            })
                        .await

                }
                output = handle_message::<T>(docker_task, compile, run, timeout, pgpool, conn, task)=> {
                match output {
                    Ok(()) => delivery.ack(BasicAckOptions::default()).await,
                    Err(e) => {
                        delivery
                            .nack(BasicNackOptions {
                                multiple: true,
                                requeue: true,
                            })
                        .await
                    }
                }
            }
            }
       }));
    }
    join_all(handles).await;
    Ok(())
}

pub struct Testcase {
    testcase: String,
    output: String,
}

async fn handle_message<T: TestcaseHandler>(
    docker_task: Docker,
    compile: Option<String>,
    run: String,
    timeout: u8,
    pgpool: sqlx::Pool<sqlx::Postgres>,
    container: Object<ContainerGroup>,
    task: WorkerTask,
) -> Result<(), ExecError> {
    let total_start = Instant::now();

    let row = sqlx::query_as!(
        Testcase,
        "SELECT testcase,output from problem_testcases WHERE problem_id=$1",
        task.problem_id as i64
    )
    .fetch_one(&pgpool)
    .await?;

    let exec_output = exec_testcase(
        docker_task,
        &container.id,
        &task.code,
        &row.testcase,
        &compile,
        &run,
        timeout,
    )
    .await?;
    T::handle_testcase(pgpool, task, row, exec_output).await?;

    let total_dur = total_start.elapsed().as_millis();
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    println!("Total duration: {} ms and {}", total_dur, ts_ms);
    Ok(())
}

pub trait TestcaseHandler {
    fn handle_testcase(
        pgpool: sqlx::Pool<sqlx::Postgres>,
        task: WorkerTask,
        row: Testcase,
        exec_output: ExecOutput,
    ) -> impl std::future::Future<Output = Result<(), ExecError>> + std::marker::Send {
        async move {
            let status: &str = match exec_output.exit_code {
                137 => ExecStatus::MemoryLimitExceeded,
                139 => ExecStatus::SegmentationFault,
                124 => ExecStatus::TimeLimitExceeded,
                _ => {
                    if are_equal_ignore_whitespace(&exec_output.output, &row.output) {
                        ExecStatus::Passed
                    } else {
                        ExecStatus::WrongAnswer
                    }
                }
            }
            .into();
            println!("task submission id is {}", task.submission_id);
            sqlx::query!(
                "UPDATE submit_status SET output=$1, status=$2 WHERE submission_id=$3",
                exec_output.output,
                status,
                task.submission_id,
            )
            .execute(&pgpool)
            .await?;
            Ok(())
        }
    }
}

pub async fn execute<T: TestcaseHandler>(
    runtime: RuntimeConfig,
    conn: lapin::Connection,
    pgpool: PgPool,
    docker: Docker,
) {
    let channel = conn.create_channel().await.expect("Error creating channel");
    let manager = ContainerGroup::new(
        docker.clone(),
        &runtime.image,
        runtime.memory,
        runtime.timeout,
    )
    .await
    .expect("Error creating Pool Manager");
    let docker_pool = Pool::builder(manager)
        .max_size(6)
        .build()
        .expect("Error creating docker pool");
    let consumer = get_consumer(&runtime.env, "code", channel)
        .await
        .expect("Unable to get consumer");

    let token = CancellationToken::new();
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
    tokio::select! {
        _ = listen::<T>(&docker, &docker_pool, pgpool, consumer, runtime.compile, runtime.run , runtime.timeout, token.clone()) => {
            docker_pool.manager().close().await;
        },
        _ = tokio::signal::ctrl_c()  => {
            token.cancel();
            docker_pool.manager().close().await;
        },
        _ = sigterm.recv() => {
            token.cancel();
            docker_pool.manager().close().await;
        }
    }
}
