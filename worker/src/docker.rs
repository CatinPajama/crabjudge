use bollard::{
    Docker,
    exec::StartExecResults,
    secret::{ContainerCreateBody, HostConfig},
};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;

pub async fn create_container(
    docker: &Docker,
    env: &str,
    memory: i64,
) -> Result<String, bollard::errors::Error> {
    let host_config = HostConfig {
        memory: Some(memory),
        memory_swap: Some(memory),
        network_mode: Some("none".to_string()),
        pids_limit: Some(16),
        security_opt: Some(vec!["no-new-privileges".to_string()]),
        ..Default::default()
    };
    let cfg = ContainerCreateBody {
        image: Some(env.to_string()),
        tty: Some(true),
        open_stdin: Some(true),
        host_config: Some(host_config),
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

pub struct ExecOutput {
    pub output: String,
    pub exit_code: i64,
}
pub async fn run_exec(
    docker: &Docker,
    id: &str,
    cmd: Vec<String>,
    testcase: &str,
) -> Result<ExecOutput, bollard::errors::Error> {
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
    let inspect_result = docker.inspect_exec(&exec_id).await?;
    let exit_code = inspect_result.exit_code.unwrap();
    Ok(ExecOutput {
        output: exec_output,
        exit_code,
    })
}
