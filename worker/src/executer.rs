use crate::docker::ExecOutput;
use crate::error::ExecError;
use crate::pool::{ContainerGroup, Pool};
use bollard::Docker;
use deadpool::managed::Object;
use futures::future::join_all;
use futures_util::StreamExt;
use lapin::{Channel, Consumer, options::*, types::FieldTable};
use models::{RuntimeConfigs, WorkerTask};
use sqlx::PgPool;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::signal::unix::{SignalKind, signal};

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
            exchange,
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
}

enum ExecStatus {
    Passed,
    WrongAnswer,
    MemoryLimitExceeded,
    SegmentationFault,
    TimeLimitExceeded,
}
impl From<ExecStatus> for &str {
    fn from(value: ExecStatus) -> Self {
        match value {
            ExecStatus::Passed => "PASSED",
            ExecStatus::WrongAnswer => "WRONG ANSWER",
            ExecStatus::MemoryLimitExceeded => "MEMORY LIMIT EXCEEDED",
            ExecStatus::SegmentationFault => "SEGMENTATION FAULT",
            ExecStatus::TimeLimitExceeded => "TIME LIMIT EXCEEDED",
        }
    }
}

async fn listen<T: TestcaseHandler>(
    docker: Docker,
    pool: Pool,
    pgpool: PgPool,
    mut consumer: Consumer,
    compile: Option<String>,
    run: String,
    timeout: u8,
) {
    let mut handles = vec![];
    while let Some(delivery) = consumer.next().await {
        let delivery = delivery.unwrap();
        let docker_task = docker.clone();
        let run = run.clone();
        let compile = compile.clone();
        let pgpool = pgpool.clone();

        let conn = pool.get().await.unwrap();
        let task: WorkerTask = serde_json::from_slice(&delivery.data)
            .map_err(|_| ExecError::ParseError)
            .unwrap();

        handles.push(tokio::spawn(async move {
            handle_message::<T>(docker_task, compile, run, timeout, pgpool, conn, task).await;
            let _ = delivery.ack(BasicAckOptions::default()).await;
        }));
    }
    join_all(handles).await;
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
) {
    let total_start = Instant::now();

    let row = sqlx::query_as!(
        Testcase,
        "SELECT testcase,output from problem_testcases WHERE problem_id=$1",
        task.problem_id as i64
    )
    .fetch_one(&pgpool)
    .await
    .unwrap();

    if let Ok(exec_output) = exec_testcase(
        docker_task,
        &container.id,
        &task.code,
        &row.testcase,
        &compile,
        &run,
        timeout,
    )
    .await
    {
        T::handle_testcase(pgpool, task, row, exec_output).await;
    }
    let total_dur = total_start.elapsed().as_millis();
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    println!("Total duration: {} ms and {}", total_dur, ts_ms);
}

pub trait TestcaseHandler {
    fn handle_testcase(
        pgpool: sqlx::Pool<sqlx::Postgres>,
        task: WorkerTask,
        row: Testcase,
        exec_output: ExecOutput,
    ) -> impl std::future::Future<Output = ()> + std::marker::Send {
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
            /*
            let status: &str = if are_equal_ignore_whitespace(&exec_output, &row.output) {
                ExecStatus::Passed
            } else {
                ExecStatus::Failed
            }
            */
            .into();
            sqlx::query!(
            "INSERT INTO submit_status (user_id, problem_id, output, status) VALUES($1,$2,$3,$4)",
            task.user_id as i64,
            task.problem_id as i64,
            exec_output.output,
            status
        )
        .execute(&pgpool)
        .await
        .unwrap();
        }
    }
}

pub async fn execute<T: TestcaseHandler>(
    runtimeconfigs: RuntimeConfigs,
    conn: lapin::Connection,
    pgpool: PgPool,
    docker: Docker,
) {
    let mut handles = vec![];
    for runtime in runtimeconfigs.runtimeconfigs {
        let docker_clone = docker.clone();
        let channel = conn.create_channel().await.unwrap();
        let pgpool_clone = pgpool.clone();
        let handle = tokio::spawn(async move {
            let manager = ContainerGroup::new(
                docker_clone.clone(),
                &runtime.1.image,
                runtime.1.memory,
                runtime.1.timeout,
            )
            .await
            .unwrap();
            let docker_pool = Pool::builder(manager).max_size(6).build().unwrap();
            let consumer = get_consumer(&runtime.0, &runtime.0, channel)
                .await
                .expect("Unable to get consumer");

            let mut sigterm =
                signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
            tokio::select! {
                _ = listen::<T>(docker_clone, docker_pool.clone(), pgpool_clone.clone(), consumer, runtime.1.compile, runtime.1.run , runtime.1.timeout) => {},
                _ = tokio::signal::ctrl_c()  => {
                    docker_pool.manager().close().await;
                },
                _ = sigterm.recv() => {
                    docker_pool.manager().close().await;
                }
            }
        });
        handles.push(handle);
    }
    join_all(handles).await;
}
