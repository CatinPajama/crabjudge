use actix_session::Session;
use actix_web::{
    HttpResponse, Responder,
    web::{Data, Form},
};
use sqlx::PgPool;

use crate::routes::{role::Role, session::SessionAuth};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SubmissionId {
    submission_id: i64,
}

#[derive(serde::Deserialize)]
pub struct FormData {
    statement: String,
    testcase: String,
    output: String,
}

pub async fn create_problem(
    pg_pool: Data<PgPool>,
    session: Session,
    form: Form<FormData>,
) -> impl Responder {
    if let Ok(Some(session_auth)) = session.get::<SessionAuth>("auth")
        && session_auth.role >= Role::ProblemSetter
    {
        let mut transaction = pg_pool.begin().await.unwrap();
        let row = sqlx::query!(
            "INSERT INTO problems (statement) VALUES($1) RETURNING problem_id",
            form.statement
        )
        .fetch_one(transaction.as_mut())
        .await;

        match row {
            Ok(row) => {
                let _row = sqlx::query!(
                    "INSERT INTO problem_testcases (problem_id,testcase,output) VALUES($1,$2,$3)",
                    row.problem_id,
                    form.testcase,
                    form.output
                )
                .fetch_one(transaction.as_mut())
                .await;
                match transaction.commit().await {
                    Ok(_) => HttpResponse::Ok().finish(),
                    Err(_) => HttpResponse::InternalServerError().finish(),
                }
            }
            Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
        }
    } else {
        HttpResponse::Unauthorized().finish()
    }
}
