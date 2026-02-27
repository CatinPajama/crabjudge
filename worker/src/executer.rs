use std::time::Duration;

use crate::docker::ExecOutput;
use crate::error::ExecError;
use crate::pool::ContainerGroup;
use backoff::{ExponentialBackoff, ExponentialBackoffBuilder};
use bollard::Docker;
use deadpool::managed::{self, Object, Pool};
use futures_util::StreamExt;
use lapin::{Channel, Consumer, ExchangeKind, options::*, types::FieldTable};
use models::{ExecStatus, RuntimeConfig, WorkerTask};
use sqlx::PgPool;
use tokio::signal::unix::{SignalKind, signal};
use tokio_util::future::FutureExt;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, error, info, instrument, warn};

fn are_equal_ignore_whitespace(s1: &str, s2: &str) -> bool {
    let s1_filtered: String = s1.chars().filter(|c| !c.is_whitespace()).collect();
    let s2_filtered: String = s2.chars().filter(|c| !c.is_whitespace()).collect();
    s1_filtered == s2_filtered
}

pub async fn declare_queue_exchange(
    channel: &Channel,
    queue: &str,
    exchange: &str,
) -> lapin::Result<()> {
    debug!("Declaring queue: {}, exchange: {}", queue, exchange);

    channel
        .queue_declare("dlq", QueueDeclareOptions::default(), FieldTable::default())
        .await?;

    channel
        .exchange_declare(
            "dlx",
            ExchangeKind::Direct,
            ExchangeDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    channel
        .queue_bind(
            "dlq",
            "dlx",
            "dlq",
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    channel
        .exchange_declare(
            exchange,
            ExchangeKind::Direct,
            ExchangeDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    let mut args = FieldTable::default();
    args.insert(
        "x-dead-letter-exchange".into(),
        lapin::types::AMQPValue::LongString("dlx".into()),
    );
    args.insert(
        "x-dead-letter-routing-key".into(),
        lapin::types::AMQPValue::LongString("dlq".into()),
    );

    channel
        .queue_declare(queue, QueueDeclareOptions::default(), args)
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
/*
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
*/
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
        // DO NOT log or record `code` - keep payloads out of logs
        code.into(),
    ];

    info!(
        "Starting exec in container {} with timeout {}s",
        container_id, timeout
    );
    match crate::docker::run_exec(&docker_task, container_id, cmd, testcase).await {
        Ok(out) => {
            info!("Exec finished with exit_code={}", out.exit_code);
            Ok(out)
        }
        Err(e) => {
            error!("Exec failed in container {}: {}", container_id, e);
            Err(ExecError::DockerError(e))
        }
    }
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

#[derive(Clone)]
pub struct CompileConfig {
    compile: Option<String>,
    run: String,
    timeout: u8,
}

async fn listen<T: TestcaseHandler>(
    task_tracker: TaskTracker,
    docker: &Docker,
    pool: &managed::Pool<ContainerGroup>,
    pgpool: PgPool,
    compile_config: CompileConfig,
    mut consumer: Consumer,

    token: CancellationToken,
) -> Result<(), ExecError> {
    while let Some(delivery) = consumer.next().await {
        let delivery = match delivery {
            Ok(d) => d,
            Err(e) => {
                error!("Error receiving delivery from queue: {}", e);
                continue;
            }
        };
        let docker_task = docker.clone();
        let pgpool = pgpool.clone();
        let compile_config = compile_config.clone();
        let conn: Object<ContainerGroup> = pool.get().await?;
        let task = serde_json::from_slice(&delivery.data);
        match task {
            Err(e) => {
                warn!("Failed to parse worker task: {}", e);
                delivery
                    .nack(BasicNackOptions {
                        multiple: false,
                        requeue: false,
                    })
                    .await?;
            }
            Ok(task) => {
                let token = token.clone();
                task_tracker.spawn(async move {
                    handle_delivery::<T>(
                        delivery,
                        docker_task,
                        pgpool,
                        compile_config,
                        conn,
                        task,
                        token,
                    )
                    .await
                });
            }
        }
    }
    task_tracker.wait().await;
    Ok(())
}

async fn handle_delivery<T: TestcaseHandler>(
    delivery: lapin::message::Delivery,
    docker_task: Docker,
    pgpool: sqlx::Pool<sqlx::Postgres>,
    compile_config: CompileConfig,
    conn: Object<ContainerGroup>,
    task: WorkerTask,
    token: CancellationToken,
) -> Result<bool, lapin::Error> {
    match handle_message::<T>(docker_task, compile_config, pgpool, conn, task)
        .with_cancellation_token_owned(token)
        .await
    {
        Some(output) => match output {
            Ok(()) => delivery.ack(BasicAckOptions::default()).await,
            Err(_) => {
                delivery
                    .nack(BasicNackOptions {
                        multiple: false,
                        requeue: false,
                    })
                    .await
            }
        },
        None => {
            delivery
                .nack(BasicNackOptions {
                    multiple: true,
                    requeue: true,
                })
                .await
        }
    }
}

pub struct Testcase {
    testcase: String,
    output: String,
}

async fn fetch_testcase(pgpool: &PgPool, problem_id: i64) -> Result<Testcase, sqlx::Error> {
    info!("Fetching testcase for problem_id {}", problem_id);
    let backoff = ExponentialBackoffBuilder::new()
        .with_max_elapsed_time(Some(Duration::from_secs(10)))
        .build();

    let res = backoff::future::retry(backoff, || async {
        Ok(sqlx::query_as!(
            Testcase,
            "SELECT testcase,output from problem_testcases WHERE problem_id=$1",
            problem_id
        )
        .fetch_one(pgpool)
        .await?)
    })
    .await;

    match res {
        Ok(tc) => {
            info!("Fetched testcase for problem_id {}", problem_id);
            Ok(tc)
        }
        Err(e) => {
            error!(
                "Failed to fetch testcase for problem_id {}: {}",
                problem_id, e
            );
            Err(e)
        }
    }
}

#[instrument(skip(docker_task, compile_config, pgpool, container, task), fields(user_id = tracing::field::Empty, submission_id = tracing::field::Empty))]
async fn handle_message<T: TestcaseHandler>(
    docker_task: Docker,
    compile_config: CompileConfig,
    pgpool: sqlx::Pool<sqlx::Postgres>,
    container: Object<ContainerGroup>,
    task: WorkerTask,
) -> Result<(), ExecError> {
    tracing::Span::current().record("user_id", &task.user_id);
    tracing::Span::current().record("submission_id", &task.submission_id);

    info!(
        "Handling task for submission_id={} problem_id={} user_id={}",
        task.submission_id, task.problem_id, task.user_id
    );

    let testcase = match fetch_testcase(&pgpool, task.problem_id).await {
        Ok(tc) => tc,
        Err(e) => {
            error!(
                "Failed to fetch testcase for submission {}: {}",
                task.submission_id, e
            );
            return Err(ExecError::DatabaseError(e));
        }
    };

    let exec_output = match exec_testcase(
        docker_task,
        &container.id,
        &task.code,
        &testcase.testcase,
        &compile_config.compile,
        &compile_config.run,
        compile_config.timeout,
    )
    .await
    {
        Ok(out) => out,
        Err(e) => {
            error!(
                "Execution failed for submission {}: {}",
                task.submission_id, e
            );
            return Err(e);
        }
    };
    let submission_id = task.submission_id;
    match T::handle_testcase(pgpool, task, testcase.output, exec_output).await {
        Ok(()) => info!(
            "Finished evaluating and updating database for submission_id={}",
            submission_id
        ),
        Err(e) => {
            error!(
                "Failed to handle testcase result for submission {}: {}",
                submission_id, e
            );
            return Err(e);
        }
    }
    Ok(())
}

async fn update_submit_status(
    pgpool: &PgPool,
    submission_id: i64,
    output: String,
    status: &str,
) -> Result<(), sqlx::Error> {
    info!(
        "Updating submit_status submission_id={} status={}",
        submission_id, status
    );
    let backoff: ExponentialBackoff = ExponentialBackoffBuilder::new()
        .with_max_elapsed_time(Some(Duration::from_secs(10)))
        .build();

    let res = backoff::future::retry(backoff, || async {
        sqlx::query!(
            "UPDATE submit_status SET output=$1, status=$2 WHERE submission_id=$3",
            output,
            status,
            submission_id,
        )
        .execute(pgpool)
        .await?;
        Ok(())
    })
    .await;

    match res {
        Ok(()) => {
            info!("Updated submit_status for submission_id={}", submission_id);
            Ok(())
        }
        Err(e) => {
            error!(
                "Failed to update submit_status for submission_id={} : {}",
                submission_id, e
            );
            Err(e)
        }
    }
}
pub trait TestcaseHandler {
    fn handle_testcase(
        pgpool: sqlx::Pool<sqlx::Postgres>,
        task: WorkerTask,
        output: String,
        exec_output: ExecOutput,
    ) -> impl std::future::Future<Output = Result<(), ExecError>> + std::marker::Send {
        async move {
            let status: &str = match exec_output.exit_code {
                137 => ExecStatus::MemoryLimitExceeded,
                139 => ExecStatus::SegmentationFault,
                124 => ExecStatus::TimeLimitExceeded,
                _ => {
                    if are_equal_ignore_whitespace(&exec_output.output, &output) {
                        ExecStatus::Passed
                    } else {
                        ExecStatus::WrongAnswer
                    }
                }
            }
            .into();
            update_submit_status(&pgpool, task.submission_id, exec_output.output, status).await?;
            Ok(())
        }
    }
}

#[instrument(skip(conn, pgpool, docker), fields(env = %runtime.env))]
pub async fn execute<T: TestcaseHandler>(
    runtime: RuntimeConfig,
    conn: lapin::Connection,
    pgpool: PgPool,
    docker: Docker,
) {
    info!("Worker started for environment: {}", runtime.env);

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
        .max_size(2)
        .build()
        .expect("Error creating docker pool");

    info!("Docker pool created with max_size: 2");

    let consumer = get_consumer(&runtime.env, "code", channel)
        .await
        .expect("Unable to get consumer");

    info!("Consumer started for environment: {}", runtime.env);

    let token = CancellationToken::new();
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");

    let compile_config = CompileConfig {
        compile: runtime.compile,
        run: runtime.run,
        timeout: runtime.timeout,
    };
    let task_tracker = TaskTracker::new();
    tokio::select! {
        _ = listen::<T>(task_tracker.clone(),&docker, &docker_pool, pgpool, compile_config, consumer, token.clone()) => {
            info!("Listen loop ended");
            docker_pool.manager().close().await;
        },
        _ = tokio::signal::ctrl_c()  => {
            info!("CTRL-C signal received, shutting down");
            task_tracker.close();
            token.cancel();
            docker_pool.manager().close().await;
        },
        _ = sigterm.recv() => {
            info!("SIGTERM signal received, shutting down");
            task_tracker.close();
            token.cancel();
            docker_pool.manager().close().await;
        }
    }
}
