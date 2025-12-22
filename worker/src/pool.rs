use bollard::Docker;
use deadpool::managed::{self, Manager};
use futures::{StreamExt as _, TryStreamExt};
use tokio::sync::Mutex;

use crate::docker::create_container;

pub struct ContainerConn {
    pub id: String,
}
pub struct ContainerGroup {
    image: String,
    docker: Docker,
    containers: Mutex<Vec<String>>,
    pub memory: i64,
    pub timeout: u8,
}

impl ContainerGroup {
    pub async fn new(
        docker: Docker,
        environment: &str,
        memory: i64,
        timeout: u8,
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
            .for_each_concurrent(None, |stream_result| async move {
                println!("Image creation: {:?}", stream_result);
            })
            .await;

        Ok(ContainerGroup {
            docker,
            image: environment.to_string(),
            containers: Mutex::new(Vec::new()),
            memory,
            timeout,
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
        let id = create_container(&self.docker, &self.image, self.memory).await?;
        self.containers.lock().await.push(id.clone());
        Ok(ContainerConn { id })
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

pub type Pool = managed::Pool<ContainerGroup>;
