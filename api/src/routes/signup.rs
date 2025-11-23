use actix_web::cookie::time::Duration;
use actix_web::web::Data;
use actix_web::{HttpRequest, HttpResponse, ResponseError, cookie::Cookie, web::Form};
use argon2::Argon2;
use argon2::PasswordHasher;
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use serde::Deserialize;
use sqlx::{Executor, PgExecutor, PgPool};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum SignupError {
    #[error("Signup Error : {0}")]
    DatabaseError(
        #[source]
        #[from]
        sqlx::Error,
    ),

    #[error("{0}")]
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

impl std::fmt::Debug for SignupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for SignupError {
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

#[derive(Deserialize)]
pub struct SignupForm {
    username: String,
    password: String,
}
pub async fn signup(
    form: Form<SignupForm>,
    pg_pool: Data<PgPool>,
) -> Result<HttpResponse, SignupError> {
    let argon2 = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = argon2
        .hash_password(form.password.as_bytes(), &salt)
        .map_err(|_| SignupError::Invalid(anyhow::anyhow!("Unabled to argon2 hash")))?
        .to_string();

    println!("{}", password_hash);
    sqlx::query!(
        r#"INSERT INTO users (username, password) VALUES ($1,$2)"#,
        form.username,
        password_hash
    )
    .execute(pg_pool.as_ref())
    .await?;

    Ok(HttpResponse::Ok().finish())
}
