use actix_session::Session;
use actix_web::{HttpResponse, Responder, web::Data};
use models::ExecStatus;
use serde::Serialize;
use sqlx::PgPool;

use crate::routes::session::SessionAuth;

#[derive(Serialize)]
struct Stats {
    difficulty: String,
    count: Option<i64>,
}

pub async fn stats(pg_pool: Data<PgPool>, session: Session) -> impl Responder {
    if let Ok(Some(SessionAuth {
        user_id,
        role: _role,
    })) = session.get::<SessionAuth>("auth")
    {
        let pass: &str = ExecStatus::Passed.into();
        let row: Result<_, sqlx::Error> = sqlx::query_as!(
            Stats,
            "SELECT difficulty, count(DISTINCT s.problem_id) FROM submit_status s INNER JOIN problems p on s.problem_id = p.problem_id WHERE status=$2 AND user_id = $1 GROUP BY difficulty",
            user_id,
            pass
        )
        .fetch_all(pg_pool.as_ref())
        .await;

        match row {
            Ok(row) => HttpResponse::Ok().json(row),
            Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
        }
    } else {
        HttpResponse::Unauthorized().finish()
    }
}
