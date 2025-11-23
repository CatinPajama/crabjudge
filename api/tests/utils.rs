use actix_web::dev::Server;
use api::{Settings, configuration, get_configuration};
use sqlx::{Connection, Executor, PgConnection, PgPool, postgres::PgConnectOptions};
use std::{io, net::TcpListener, sync::Arc};
use uuid::Uuid;

pub struct TestApp {
    pub port: u16,
}


pub async fn spawn_app() -> TestApp {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Unable to start tcp listener");

    let settings = get_configuration().expect("Unable to read configuration");
    let pg_options = PgConnectOptions::new()
        .database("postgres")
        .username(&settings.database.user)
        .host(&settings.database.host)
        .password(&settings.database.password);
    let mut conn = PgConnection::connect_with(&pg_options)
        .await
        .expect("Unable to connect to postgres");

    let dbname = Uuid::new_v4().to_string();
    let dbname_query = format!(r#"CREATE DATABASE "{}";"#, dbname);
    conn.execute(dbname_query.as_str())
        .await
        .expect("failed to create database");

    let pg_options = PgConnectOptions::new()
        .database(&dbname)
        .host(&settings.database.host)
        .username(&settings.database.user)
        .password(&settings.database.password);

    let pg_pool = PgPool::connect_with(pg_options)
        .await
        .expect("Unable to connect to new database");

    sqlx::migrate!("../migrations")
        .run(&pg_pool)
        .await
        .expect("Failed to run migrations on test database");

    let app = TestApp {
        port: listener.local_addr().unwrap().port(),
    };
    let server = api::run(pg_pool, listener)
        .await
        .expect("Unable to run the app");
    let _ = tokio::spawn(async {
        server.await.expect("Unable to start server");
    });
    app
}
