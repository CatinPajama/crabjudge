use api::{Application, startup};
use models::Settings;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::get_configuration().expect("Unable to read configuration files");

    let app = Application::build(settings).await?;

    // todo!()
    app.run_until_stopped().await?;

    Ok(())
}
