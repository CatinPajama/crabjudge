use actix_session::Session;
use anyhow::Context;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use base64::{Engine as _, engine::general_purpose};
use std::future::{Ready, ready};
use uuid::Uuid;

use actix_web::{
    FromRequest, HttpResponse, ResponseError,
    cookie::{Cookie, time::Duration},
    http::header::HeaderValue,
    web::Data,
};
use sqlx::{Executor, PgPool};

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Login Error : {0}")]
    DatabaseError(
        #[source]
        #[from]
        sqlx::Error,
    ),

    #[error(transparent)]
    Invalid(#[from] anyhow::Error),

    #[error(transparent)]
    SessionInsertError(#[from] actix_session::SessionInsertError),

    #[error(transparent)]
    SessionGetError(#[from] actix_session::SessionGetError),
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

impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for LoginError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        match self {
            Self::DatabaseError(_) => HttpResponse::InternalServerError().finish(),
            Self::Invalid(e) => {
                let cookie = Cookie::build("login_error", e.to_string())
                    .max_age(Duration::seconds(0))
                    .finish();
                HttpResponse::BadRequest().cookie(cookie).finish()
            }
            Self::SessionGetError(e) => e.error_response(),
            Self::SessionInsertError(_) => HttpResponse::InternalServerError().finish(),
        }
    }
}

pub struct Credentials {
    pub username: String,
    pub password: String, // TODO use secret package
}

fn extract_credentials(auth_header: &HeaderValue) -> Result<Credentials, LoginError> {
    let auth_str = auth_header.to_str().context("Non Ascii not allowed")?;

    let base64_encoded =
        auth_str
            .strip_prefix("Basic ")
            .ok_or(LoginError::Invalid(anyhow::anyhow!(
                "Invalid basic auth format"
            )))?;

    let base64_decoded_bytes = general_purpose::STANDARD
        .decode(base64_encoded)
        .map_err(|_| LoginError::Invalid(anyhow::anyhow!("Invalid base64 encoded")))?;

    let base64_decoded_string =
        String::from_utf8(base64_decoded_bytes).context("Decoded string is not valid utf8")?;
    let (username, password) = base64_decoded_string.split_at(
        base64_decoded_string
            .find(':')
            .ok_or(anyhow::anyhow!("No colon separating username and password"))?,
    );

    Ok(Credentials {
        username: username.to_string(),
        password: password[1..].to_string(),
    })
}

impl FromRequest for Credentials {
    type Error = LoginError;
    type Future = Ready<Result<Self, Self::Error>>;
    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        let result = req.headers().get("Authorization");

        match result {
            None => ready(Err(LoginError::Invalid(anyhow::anyhow!(
                "No authorization key"
            )))),
            Some(auth_header) => ready(extract_credentials(auth_header)),
        }
    }
}

pub async fn login(
    credentials: Credentials,
    pgpool: Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, LoginError> {
    let row = sqlx::query!(
        r#"SELECT user_id,password from users  WHERE username=$1 ;"#,
        credentials.username,
    )
    .fetch_one(pgpool.as_ref())
    .await?;

    let argon2 = Argon2::default();

    let phc = PasswordHash::new(&row.password)
        .map_err(|_| LoginError::Invalid(anyhow::anyhow!("Wrong phc stored")))?;

    match argon2.verify_password(credentials.password.as_bytes(), &phc) {
        Ok(_) => {
            if let Ok(Some(user_id)) = session.get::<i64>("user_id") {
            } else {
                println!("putting in session {}", row.user_id);
                session.insert("user_id", row.user_id).unwrap();
            }
            Ok(HttpResponse::Ok().finish())
        }
        Err(_) => Ok(HttpResponse::Unauthorized().finish()),
    }
}
