use actix_web::{
    HttpResponse, Responder,
    web::{self, Data},
};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Serialize)]
struct Problem {
    problem_id: i64,
    title: String,
    difficulty: String,
    statement: String,
}

pub async fn list_problems(pg_pool: Data<PgPool>) -> impl Responder {
    let rows: Result<_, sqlx::Error> = sqlx::query_as!(
        Problem,
        "SELECT problem_id, title, difficulty, statement FROM problems ORDER BY problem_id"
    )
    .fetch_all(pg_pool.as_ref())
    .await;

    match rows {
        Ok(rows) => HttpResponse::Ok().json(rows),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

pub async fn problem(pg_pool: Data<PgPool>, path: web::Path<(i64,)>) -> impl Responder {
    let problem_id = path.into_inner().0;
    let problem: Result<_, sqlx::Error> = sqlx::query_as!(
        Problem,
        "SELECT problem_id, title, difficulty, statement FROM problems WHERE problem_id = $1",
        problem_id,
    )
    .fetch_one(pg_pool.as_ref())
    .await;

    match problem {
        Ok(p) => HttpResponse::Ok().json(p),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
