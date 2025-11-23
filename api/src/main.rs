use api::{Application, configuration, startup};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = configuration::get_configuration().expect("Unable to read configuration files");

    let app = Application::build(settings).await?;

    // todo!()
    app.run_until_stopped().await?;

    Ok(())
}
