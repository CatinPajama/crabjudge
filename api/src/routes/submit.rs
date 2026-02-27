use actix_session::Session;
use actix_web::{
    HttpResponse, ResponseError,
    web::{self, Data},
};
use tracing::{error, info, instrument, warn};
use validator::Validate;

use lapin::{
    BasicProperties,
    options::{BasicPublishOptions, ExchangeDeclareOptions},
    types::FieldTable,
};
use models::{RuntimeConfigs, WorkerTask};
use serde_json::json;
use sqlx::PgPool;

use crate::routes::session::SessionAuth;

#[derive(serde::Deserialize, Validate)]
pub struct SubmitJson {
    #[validate(length(min = 1, message = "Code cannot be empty"))]
    code: String,
    #[validate(length(min = 1, max = 50, message = "Environment must be specified"))]
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

    #[error("Validation error: {0}")]
    Validation(#[from] anyhow::Error),
}

impl ResponseError for SubmitError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        match self {
            Self::InvalidEnvironment(env) => HttpResponse::BadRequest().body(env.clone()),
            Self::Validation(e) => HttpResponse::BadRequest().body(e.to_string()),
            Self::QueueError(e) => HttpResponse::InternalServerError().body(e.to_string()),
            Self::DatabaseError(e) => HttpResponse::InternalServerError().body(e.to_string()),
        }
    }
}

#[instrument(skip(request, conn, session, runtimeconfigs, pg_pool), fields(user_id = tracing::field::Empty))]
pub async fn submit_problem(
    request: web::Json<SubmitJson>,
    path: web::Path<(i64,)>,
    conn: Data<lapin::Connection>,
    session: Session,
    runtimeconfigs: Data<RuntimeConfigs>,
    pg_pool: Data<PgPool>,
) -> Result<HttpResponse, SubmitError> {
    info!("Submission attempt initiated");

    // validate payload
    request.validate().map_err(|e| {
        warn!("Submission validation failed: {}", e);
        SubmitError::Validation(anyhow::anyhow!(e))
    })?;

    // sanitize code size
    if let Ok(Some(auth)) = session.get::<SessionAuth>("auth") {
        // attach user id to the current span for easier tracing/searching
        tracing::Span::current().record("user_id", &auth.user_id);
        if !runtimeconfigs.0.contains_key(&request.env) {
            warn!("Invalid environment: {}", request.env);
            return Ok(HttpResponse::BadRequest().body("Invalid environment"));
        }

        let problem_id = path.into_inner().0;
        info!(
            "Submission for problem_id: {}, user_id: {}, env: {}",
            problem_id, auth.user_id, request.env
        );

        let channel = conn.create_channel().await?;

        let submission_id = sqlx::query!(
            r#"INSERT INTO submit_status (user_id, problem_id) VALUES ($1,$2) RETURNING submission_id"#,
            auth.user_id,
            problem_id,
        )
        .fetch_one(pg_pool.as_ref())
        .await
        .map_err(|e| {
            error!("Failed to create submission record: {}", e);
            SubmitError::DatabaseError(e)
        })?.submission_id;

        info!("Submission record created with id: {}", submission_id);

        let worker_task = WorkerTask {
            code: request.code.clone(),
            problem_id,
            user_id: auth.user_id,
            submission_id,
        };

        channel
            .exchange_declare(
                "code",
                lapin::ExchangeKind::Direct,
                ExchangeDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                error!("Failed to declare exchange: {}", e);
                SubmitError::QueueError(e)
            })?;

        channel
            .basic_publish(
                "code",
                &request.env,
                BasicPublishOptions::default(),
                serde_json::to_vec(&worker_task).unwrap().as_ref(),
                BasicProperties::default(),
            )
            .await
            .map_err(|e| {
                error!("Failed to publish to queue: {}", e);
                SubmitError::QueueError(e)
            })?;

        info!(
            "Submission published to queue successfully. submission_id: {}",
            submission_id
        );

        Ok(HttpResponse::Ok().json(json!({
            "submission_id" : submission_id
        })))
    } else {
        warn!("Unauthorized submission attempt");
        Ok(HttpResponse::Unauthorized().finish())
    }
}
