use std::{net::TcpListener, u16};

use crate::routes::signup;
use crate::routes::{Credentials, login, status, submissions, submit, submit_problem};
use actix_session::config::SessionMiddlewareBuilder;
use actix_session::storage::RedisSessionStore;
use actix_session::{Session, SessionMiddleware};
use actix_web::HttpResponse;
use actix_web::body::MessageBody;
use actix_web::cookie::Key;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{
    App,
    dev::Server,
    web::{self, Data},
};
use models::Settings;
use sqlx::PgPool;

pub struct Application {
    port: u16,
    host: String,
    server: Server,
}

pub struct ApplicationBaseUrl(String);

impl Application {
    pub async fn build(settings: Settings) -> Result<Self, anyhow::Error> {
        let address = format!(
            "{}:{}",
            settings.application.host, settings.application.port
        );

        let pgpool = PgPool::connect_lazy(&settings.database.url())?;

        let listener = TcpListener::bind(address)?;
        let redis_store = RedisSessionStore::new(settings.redis.url()).await.unwrap();
        let rabbitmq_conn = lapin::Connection::connect(
            &settings.rabbitmq.url(),
            lapin::ConnectionProperties::default(),
        )
        .await?;
        let server = run(pgpool, listener, redis_store, rabbitmq_conn).await?;
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

pub async fn run(
    pgpool: PgPool,
    listener: TcpListener,
    redis_store: RedisSessionStore,
    rabbitmq_conn: lapin::Connection,
) -> Result<Server, anyhow::Error> {
    let data_pgpool = Data::new(pgpool);
    let data_rabbitmq = Data::new(rabbitmq_conn);
    let secret_key = Key::generate();

    let server = actix_web::HttpServer::new(move || {
        App::new()
            .wrap(
                SessionMiddleware::builder(redis_store.clone(), secret_key.clone())
                    .cookie_secure(false)
                    .cookie_http_only(true)
                    .build(),
            )
            .app_data(data_pgpool.clone())
            .app_data(data_rabbitmq.clone())
            .route("/login", web::post().to(login))
            .route("/signup", web::post().to(signup))
            .route("/{problemID}/submit", web::post().to(submit_problem))
            .route("/{submissionID}/status", web::get().to(status))
            .route("/{problemID}/submissions", web::get().to(submissions))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
