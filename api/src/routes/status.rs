use actix_session::Session;
use actix_web::{
    HttpResponse, Responder,
    web::{self, Data},
};
use serde::Serialize;
use sqlx::Executor;
use sqlx::PgPool;

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
    if let Ok(Some(user_id)) = session.get::<i64>("user_id") {
        let submission_id = path.into_inner().0;

        let row: Status = sqlx::query_as!(
            Status,
            "SELECT user_id,problem_id,status,output from submit_status WHERE submission_id = $1",
            submission_id
        )
        .fetch_one(pg_pool.as_ref())
        .await
        .unwrap();
        HttpResponse::Ok().json(row)
    } else {
        HttpResponse::Unauthorized().finish()
    }
}
