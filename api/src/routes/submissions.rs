use actix_session::Session;
use actix_web::{
    HttpResponse, Responder,
    web::{self, Data},
};
use sqlx::PgPool;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SubmissionId {
    submission_id: i64,
}

pub async fn submissions(
    pg_pool: Data<PgPool>,
    session: Session,
    path: web::Path<(i64,)>,
) -> impl Responder {
    if let Ok(Some(user_id)) = session.get::<i64>("user_id") {
        let problem_id = path.into_inner().0;
        let row = sqlx::query_as!(
            SubmissionId,
            "SELECT submission_id from submit_status WHERE user_id = $1 AND problem_id = $2",
            user_id,
            problem_id
        )
        .fetch_all(pg_pool.as_ref())
        .await
        .unwrap();
        HttpResponse::Ok().json(row)
    } else {
        HttpResponse::Unauthorized().finish()
    }
}
