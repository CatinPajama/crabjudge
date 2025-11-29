use bollard::{Docker, exec::StartExecResults, secret::ContainerCreateBody};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;

pub async fn create_container(
    docker: &Docker,
    env: &str,
) -> Result<String, bollard::errors::Error> {
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

pub async fn run_exec(
    docker: &Docker,
    id: &str,
    cmd: Vec<String>,
    testcase: &str,
) -> Result<String, bollard::errors::Error> {
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
