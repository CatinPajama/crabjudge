use std::net::TcpListener;

use crate::routes::create_problem::post::create_problem;
use crate::routes::{
    list_problems, login, signup_confirmation, stats, status, submissions, submit_problem,
};
use crate::routes::{problem, signup};
use actix_cors::Cors;
use actix_session::SessionMiddleware;
use actix_session::storage::RedisSessionStore;
use actix_web::cookie::Key;
use actix_web::{
    App,
    dev::Server,
    web::{self, Data},
};
use models::email::EmailClient;
use models::{RuntimeConfigs, Settings};
use sqlx::PgPool;

#[allow(dead_code)]
pub struct Application {
    port: u16,
    host: String,
    server: Server,
}

impl Application {
    pub async fn build(settings: Settings) -> Result<Self, anyhow::Error> {
        let address = format!(
            "{}:{}",
            settings.application.host, settings.application.port
        );

        let pgpool = PgPool::connect_lazy(&settings.database.url())?;

        let listener = TcpListener::bind(address)?;
        let redis_store = RedisSessionStore::new(settings.redis.url()).await.unwrap();

        let sender_email = settings
            .email_client
            .sender()
            .expect("Invalid sender email address");
        let email_client = EmailClient::new(
            settings.email_client.base_url,
            sender_email,
            settings.email_client.authorization_token,
        );
        let rabbitmq_conn = lapin::Connection::connect(
            &settings.rabbitmq.url(),
            lapin::ConnectionProperties::default(),
        )
        .await?;
        let server = run(
            pgpool,
            listener,
            redis_store,
            rabbitmq_conn,
            email_client,
            settings.runtimeconfigs,
            settings.application.base_url,
        )
        .await?;
        Ok(Application {
            host: settings.application.host,
            port: settings.application.port,
            server,
        })
    }
    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub struct ApplicationBaseUrl(pub String);

pub async fn run(
    pgpool: PgPool,
    listener: TcpListener,
    redis_store: RedisSessionStore,
    rabbitmq_conn: lapin::Connection,
    email_client: EmailClient,
    runtimeconfigs: RuntimeConfigs,
    base_url: String,
) -> Result<Server, anyhow::Error> {
    let data_pgpool = Data::new(pgpool);
    let data_rabbitmq = Data::new(rabbitmq_conn);
    let data_runtimeconfigs = Data::new(runtimeconfigs);
    let email_client = Data::new(email_client);
    let application_base_url = Data::new(ApplicationBaseUrl(base_url));
    let secret_key = Key::generate();

    let server = actix_web::HttpServer::new(move || {
        println!("started");
        let cors = Cors::default()
            .allowed_origin("http://127.0.0.1:5173") // Replace with your frontend's origin
            .allowed_origin("http://localhost:5173")
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(&[
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::ACCEPT,
            ])
            .allowed_header(actix_web::http::header::CONTENT_TYPE)
            .supports_credentials()
            .max_age(3600);
        App::new()
            .wrap(cors)
            .wrap(
                SessionMiddleware::builder(redis_store.clone(), secret_key.clone())
                    .cookie_secure(false)
                    .cookie_http_only(true)
                    .build(),
            )
            .app_data(data_pgpool.clone())
            .app_data(data_rabbitmq.clone())
            .app_data(data_runtimeconfigs.clone())
            .app_data(email_client.clone())
            .app_data(application_base_url.clone())
            .route("/login", web::post().to(login))
            .route("/signup", web::post().to(signup))
            .route("/signup/confirmation", web::get().to(signup_confirmation))
            .route("/{problemID}/submit", web::post().to(submit_problem))
            .route("/{submissionID}/status", web::get().to(status))
            .route("/problem/{problemID}", web::get().to(problem))
            .route("/{problemID}/submissions", web::get().to(submissions))
            .route("/createProblem", web::post().to(create_problem))
            .route("/problems", web::get().to(list_problems))
            .route("/stats", web::get().to(stats))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
