use actix_session::Session;
use anyhow::Context;
use argon2::{
    Argon2, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
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
#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: String,
}

pub async fn signup_confirmation(
    form: web::Form<FormData>,
    query_params: web::Query<ConfirmationQueryParams>,
    pgpool: Data<PgPool>,
) -> Result<HttpResponse, ConfirmationError> {

    let mut tx = pgpool.begin().await?;

    let row = sqlx::query!(
        r#"DELETE FROM verification WHERE token= $1 AND created_at > NOW() - INTERVAL '7 days' RETURNING email "#,
        query_params.verification_token
    )
    .fetch_optional(&mut *tx)
    .await?;

    if row.is_none() {
        return Ok(HttpResponse::BadRequest().body("Invalid confirmation"));
    }
    let argon2 = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = argon2
        .hash_password(form.password.as_bytes(), &salt)
        .map_err(|_| ConfirmationError::Invalid(anyhow::anyhow!("Unabled to argon2 hash")))?
        .to_string();

    let role : &str = Role::User.into();
    sqlx::query!(
        r#"INSERT INTO users (username, password, role, email) VALUES ($1,$2,$3,$4);"#,
        form.username,
        password_hash,
        role,
        row.unwrap().email
    ).execute(&mut *tx).await?;

    tx.commit().await?;

    Ok(HttpResponse::Ok().finish())
}
