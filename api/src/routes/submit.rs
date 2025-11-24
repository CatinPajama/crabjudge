use actix_session::Session;
use actix_web::{
    HttpResponse, Responder,
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

pub async fn submit_problem(
    request: web::Json<SubmitJson>,
    path: web::Path<(i64,)>,
    conn: Data<lapin::Connection>,
    session: Session,
) -> impl Responder {
    // sanitize code size
    if let Ok(Some(user_id)) = session.get::<i64>("user_id") {
        let problem_id = path.into_inner().0;
        let channel = conn.create_channel().await.unwrap();

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
            .await
            .unwrap();
        channel
            .basic_publish(
                &request.env,
                "",
                BasicPublishOptions::default(),
                serde_json::to_vec(&worker_task).unwrap().as_ref(),
                BasicProperties::default(),
            )
            .await
            .unwrap();
        HttpResponse::Ok().body(format!("Submitting problem with ID: {}", problem_id))
    } else {
        HttpResponse::Unauthorized().finish()
    }
}
