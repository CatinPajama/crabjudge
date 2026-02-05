use actix_session::Session;
use actix_web::{
    HttpResponse, Responder,
    web::{self, Data},
};
use serde::Serialize;
use sqlx::PgPool;

use crate::routes::session::SessionAuth;

#[derive(Serialize)]
struct Status {
    user_id: i64,
    problem_id: i64,
    status: String,
    output: String,
}
pub async fn status(
    path: web::Path<(i64,)>,
    pg_pool: Data<PgPool>,
    session: Session,
) -> impl Responder {
    if let Ok(Some(_auth)) = session.get::<SessionAuth>("auth") {
        let submission_id = path.into_inner().0;

        let row: Result<Status, sqlx::Error> = sqlx::query_as!(
            Status,
            "SELECT user_id,problem_id,status,output from submit_status WHERE submission_id = $1",
            submission_id
        )
        .fetch_one(pg_pool.as_ref())
        .await;

        match row {
            Ok(row) => HttpResponse::Ok().json(row),
            Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
        }
    } else {
        HttpResponse::Unauthorized().finish()
    }
}
