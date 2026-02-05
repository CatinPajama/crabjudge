use actix_web::{HttpResponse, Responder, web::Data};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Serialize)]
struct Problem {
    problem_id: i64,
    statement: String,
}

pub async fn list_problems(pg_pool: Data<PgPool>) -> impl Responder {
    let rows = sqlx::query_as!(
        Problem,
        "SELECT problem_id, statement FROM problems ORDER BY problem_id"
    )
    .fetch_all(pg_pool.as_ref())
    .await;

    match rows {
        Ok(rows) => HttpResponse::Ok().json(rows),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
