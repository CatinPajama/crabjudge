use actix_session::Session;
use anyhow::Context;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use base64::{Engine as _, engine::general_purpose};
use std::future::{Ready, ready};

use actix_web::{
    FromRequest, HttpResponse, ResponseError,
    cookie::{Cookie, time::Duration},
    http::header::HeaderValue,
    web::{self, Data},
};
use sqlx::PgPool;

use crate::routes::{role::Role, session::SessionAuth};

#[derive(thiserror::Error)]
pub enum ConfirmationError {
    #[error("Login Error : {0}")]
    DatabaseError(
        #[source]
        #[from]
        sqlx::Error,
    ),

    #[error(transparent)]
    Invalid(#[from] anyhow::Error),
}
fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

impl std::fmt::Debug for ConfirmationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ConfirmationError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        match self {
            Self::DatabaseError(_) => HttpResponse::InternalServerError().finish(),
            Self::Invalid(e) => {
                let cookie = Cookie::build("login_error", e.to_string())
                    .max_age(Duration::seconds(0))
                    .finish();
                HttpResponse::BadRequest().cookie(cookie).finish()
            }
        }
    }
}
#[derive(serde::Deserialize)]
pub struct ConfirmationQueryParams {
    verification_token: String,
}

pub async fn signup_confirmation(
    query_params: web::Query<ConfirmationQueryParams>,
    pgpool: Data<PgPool>,
) -> Result<HttpResponse, ConfirmationError> {
    let row = sqlx::query!(
        r#"UPDATE users SET verification_token=NULL WHERE verification_token= $1"#,
        query_params.verification_token
    )
    .execute(pgpool.as_ref())
    .await?;

    if row.rows_affected() == 0 {
        return Ok(HttpResponse::BadRequest().body("Invalid confirmation"));
    }

    Ok(HttpResponse::Ok().finish())
}
