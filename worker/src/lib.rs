mod configuration;
mod manager;

use bollard::{
    Docker,
    container::RemoveContainerOptions,
    errors::Error as DockerError,
    exec::StartExecResults,
    query_parameters::{self, CreateImageOptions, CreateImageOptionsBuilder},
    secret::ContainerCreateBody,
};
use deadpool::managed::{self, Manager};
use futures::TryStreamExt;
use futures_util::stream::StreamExt;
use tokio::{io::AsyncWriteExt, sync::Mutex, task::JoinHandle};

pub struct ContainerConn {
    pub id: String,
    docker: Docker,
}
/*
impl ContainerConn {
    async fn drop(&mut self) {
        let docker = self.docker.clone();
        let id = self.id.clone();

        tokio::spawn(async move {});
    }
}
*/

async fn create_container(docker: &Docker, env: &str) -> Result<String, DockerError> {
    let cfg = ContainerCreateBody {
        image: Some(env.to_string()),
        tty: Some(true),
        ..Default::default()
    };

    let id = docker
        .create_container(
            None::<bollard::query_parameters::CreateContainerOptions>,
            cfg,
        )
        .await?
        .id;

    docker
        .start_container(
            &id,
            None::<bollard::query_parameters::StartContainerOptions>,
        )
        .await?;

    Ok(id)
}

pub struct ContainerGroup {
    image: String,
    docker: Docker,
    containers: Mutex<Vec<String>>,
}

impl ContainerGroup {
    pub async fn new(
        docker: Docker,
        environment: &str,
    ) -> Result<ContainerGroup, bollard::errors::Error> {
        docker
            .create_image(
                Some(
                    bollard::query_parameters::CreateImageOptionsBuilder::default()
                        .from_image(environment)
                        .build(),
                ),
                None,
                None,
            )
            .try_collect::<Vec<_>>()
            .await?;
        Ok(ContainerGroup {
            docker,
            image: environment.to_string(),
            containers: Mutex::new(Vec::new()),
        })
    }
    pub async fn close(&self) {
        for container in self.containers.lock().await.iter() {
            let _ = self
                .docker
                .kill_container(
                    container,
                    Some(bollard::query_parameters::KillContainerOptions::default()),
                )
                .await;

            let _ = self
                .docker
                .remove_container(
                    container,
                    Some(bollard::query_parameters::RemoveContainerOptions::default()),
                )
                .await;
        }
    }
}

impl Manager for ContainerGroup {
    type Type = ContainerConn;
    type Error = bollard::errors::Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        let id = create_container(&self.docker, &self.image).await?;
        self.containers.lock().await.push(id.clone());
        Ok(ContainerConn {
            id,
            docker: self.docker.clone(),
        })
    }
    async fn recycle(
        &self,
        conn: &mut Self::Type,
        _: &managed::Metrics,
    ) -> managed::RecycleResult<Self::Error> {
        self.docker
            .inspect_container(
                &conn.id,
                None::<bollard::query_parameters::InspectContainerOptions>,
            )
            .await
            .map(|_| ())
            .map_err(managed::RecycleError::from)
    }
}
/*
pub async fn run_exec(
    docker: &Docker,
    id: &str,
    cmd: Vec<String>,
    testcase: String,
) -> Result<String, DockerError> {
    let exec_id = docker
        .create_exec(
            id,
            bollard::models::ExecConfig {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(cmd),
                ..Default::default()
            },
        )
        .await?
        .id;

    let mut exec_output = String::new();
    if let bollard::exec::StartExecResults::Attached { mut output, .. } =
        docker.start_exec(&exec_id, None).await?
    {
        while let Some(Ok(msg)) = output.next().await {
            exec_output.push_str(&msg.to_string());
        }
    }
    Ok(exec_output)
}
*/
pub async fn run_exec(
    docker: &Docker,
    id: &str,
    cmd: Vec<String>,
    testcase: String,
) -> Result<String, DockerError> {
    let exec_id = docker
        .create_exec(
            id,
            bollard::models::ExecConfig {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                attach_stdin: Some(true),
                cmd: Some(cmd),
                ..Default::default()
            },
        )
        .await?
        .id;

    let mut exec_output = String::new();

    if let StartExecResults::Attached { mut output, input } =
        docker.start_exec(&exec_id, None).await?
    {
        let mut input_stream = input;

        input_stream.write_all(testcase.as_bytes()).await?;

        input_stream.shutdown().await?;

        while let Some(Ok(msg)) = output.next().await {
            exec_output.push_str(&msg.to_string());
        }
    } else {
        // TODO handle detach case
    }

    Ok(exec_output)
}
pub type Pool = managed::Pool<ContainerGroup>;
/*
#[tokio::test]
async fn test_docker() -> Result<(), Box<dyn std::error::Error>> {
    let docker = Docker::connect_with_local_defaults()?;

    let manager = ContainerGroup::new(docker.clone(), "python:3.12-slim").await?;

    let pool = Pool::builder(manager).max_size(3).build()?;

    let mut handles: Vec<JoinHandle<()>> = vec![];

    for i in 0..5 {
        let d = docker.clone();
        let conn = pool.get().await.unwrap();

        let cmd: Vec<String> = vec![
            "python".to_string(),
            "-c".to_string(),
            format!("import time; time.sleep({}); print('hi')", 5 - i + 1),
        ];
        handles.push(tokio::spawn(async move {
            println!("Exec in container {}", conn.id);
            run_exec(&d, &conn.id, cmd).await.unwrap();
        }));
    }
    futures::future::join_all(handles).await;
    pool.manager().close().await;
    println!("All tasks done. Containers cleaned automatically.");
    Ok(())
}
*/
