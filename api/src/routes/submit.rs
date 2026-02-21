use actix_session::Session;
use actix_web::{
    HttpResponse, ResponseError,
    web::{self, Data},
};

use lapin::{
    BasicProperties,
    options::{BasicPublishOptions, ExchangeDeclareOptions},
    types::FieldTable,
};
use models::{RuntimeConfigs, WorkerTask};
use serde_json::json;
use sqlx::PgPool;

use crate::routes::session::SessionAuth;

#[derive(serde::Deserialize)]
pub struct SubmitJson {
    code: String,
    env: String,
}

#[derive(thiserror::Error, Debug)]
pub enum SubmitError {
    #[error("{0}")]
    QueueError(#[from] lapin::Error),

    #[error("{0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("No such environment {0}")]
    InvalidEnvironment(String),
}

impl ResponseError for SubmitError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        match self {
            Self::InvalidEnvironment(env) => HttpResponse::BadRequest().body(env.clone()),
            Self::QueueError(e) => HttpResponse::InternalServerError().body(e.to_string()),
            Self::DatabaseError(e) => HttpResponse::InternalServerError().body(e.to_string()),
        }
    }
}

pub async fn submit_problem(
    request: web::Json<SubmitJson>,
    path: web::Path<(i64,)>,
    conn: Data<lapin::Connection>,
    session: Session,
    runtimeconfigs: Data<RuntimeConfigs>,
    pg_pool: Data<PgPool>,
) -> Result<HttpResponse, SubmitError> {
    // sanitize code size
    if let Ok(Some(auth)) = session.get::<SessionAuth>("auth") {
        if !runtimeconfigs.0.contains_key(&request.env) {
            return Ok(HttpResponse::BadRequest().body("Invalid environment"));
        }
        let problem_id = path.into_inner().0;
        let channel = conn.create_channel().await?;

        let submission_id = sqlx::query!(
            r#"INSERT INTO submit_status (user_id, problem_id) VALUES ($1,$2) RETURNING submission_id"#,
            auth.user_id,
            problem_id,
        )
        .fetch_one(pg_pool.as_ref())
        .await?.submission_id;

        let worker_task = WorkerTask {
            code: request.code.clone(),
            problem_id,
            user_id: auth.user_id,
            submission_id,
        };
        channel
            .exchange_declare(
                "code",
                // &request.env,
                lapin::ExchangeKind::Direct,
                ExchangeDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;
        channel
            .basic_publish(
                "code",
                &request.env,
                // "",
                BasicPublishOptions::default(),
                serde_json::to_vec(&worker_task).unwrap().as_ref(),
                BasicProperties::default(),
            )
            .await?;
        Ok(HttpResponse::Ok().json(json!({
            "submission_id" : submission_id
        })))
    } else {
        Ok(HttpResponse::Unauthorized().finish())
    }
}
