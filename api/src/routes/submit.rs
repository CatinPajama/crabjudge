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
use models::WorkerTask;

#[derive(serde::Deserialize)]
pub struct SubmitJson {
    code: String,
    env: String,
}

#[derive(thiserror::Error, Debug)]
pub enum SubmitError {
    #[error("{0}")]
    QueueError(#[from] lapin::Error),

    #[error("No such environment {0}")]
    InvalidEnvironment(String),
}

impl ResponseError for SubmitError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        match self {
            Self::InvalidEnvironment(env) => HttpResponse::BadRequest().body(env.clone()),
            Self::QueueError(e) => HttpResponse::InternalServerError().body(e.to_string()),
        }
    }
}

pub async fn submit_problem(
    request: web::Json<SubmitJson>,
    path: web::Path<(i64,)>,
    conn: Data<lapin::Connection>,
    session: Session,
) -> Result<HttpResponse, SubmitError> {
    // sanitize code size
    if let Ok(Some(user_id)) = session.get::<i64>("user_id") {
        let problem_id = path.into_inner().0;
        let channel = conn.create_channel().await?;

        let worker_task = WorkerTask {
            code: request.code.clone(),
            problem_id,
            user_id,
        };
        channel
            .exchange_declare(
                &request.env,
                lapin::ExchangeKind::Direct,
                ExchangeDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;
        channel
            .basic_publish(
                &request.env,
                "",
                BasicPublishOptions::default(),
                serde_json::to_vec(&worker_task).unwrap().as_ref(),
                BasicProperties::default(),
            )
            .await?;
        Ok(HttpResponse::Ok().body(format!("Submitting problem with ID: {}", problem_id)))
    } else {
        Ok(HttpResponse::Unauthorized().finish())
    }
}
