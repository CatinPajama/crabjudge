use actix_web::cookie::time::Duration;
use actix_web::web::Data;
use actix_web::{HttpResponse, ResponseError, cookie::Cookie, web::Form};
use models::email::{EmailClient, SubscriberEmail};
use rand::RngExt;
use rand::distr::Alphanumeric;
use serde::Deserialize;
use sqlx::PgPool;

use crate::ApplicationBaseUrl;

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

    #[error("0")]
    EmailError(#[from] reqwest::Error),
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
            Self::DatabaseError(e) => HttpResponse::InternalServerError().body(e.to_string()),
            Self::Invalid(e) => {
                let cookie = Cookie::build("signup_error", e.to_string())
                    .max_age(Duration::seconds(0))
                    .finish();
                HttpResponse::BadRequest().cookie(cookie).finish()
            }
            Self::EmailError(e) => HttpResponse::InternalServerError().body(e.to_string()),
        }
    }
}

#[derive(Deserialize)]
pub struct SignupForm {
    email: String,
}

fn generate_verification_token() -> String {
    let mut rng = rand::rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

pub async fn signup(
    form: Form<SignupForm>,
    pg_pool: Data<PgPool>,
    email_client: Data<EmailClient>,
    application_base_url: Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, SignupError> {
    let receiver_email = SubscriberEmail::parse(form.email.to_owned())
        .map_err(|e| SignupError::Invalid(anyhow::anyhow!(e)))?;
    let verification_token = generate_verification_token();
    sqlx::query!(
        r#"INSERT INTO verification (email, token_type, token) VALUES ($1,'signup',$2) ON CONFLICT(email,token_type) DO UPDATE SET token = EXCLUDED.token, created_at = NOW();"#,
        form.email,
        verification_token,
    )
    .execute(pg_pool.as_ref())
    .await?;

    let text = format!(
        r#"Click the link to confirm your signup <a href="{}/verify?verification_token={}">Click me</a>"#,
        application_base_url.0, verification_token
    );
    email_client
        .send_email(receiver_email, "Email Signup Confirmation", &text, &text)
        .await
        .map_err(SignupError::EmailError)?;

    Ok(HttpResponse::Ok().body("Verification link sent"))
}
