// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::auth::AuthenticatedUser;
use crate::auth::cognito::{SignInOutcome, TokenSet};
use crate::auth::jwt;
use crate::auth::session::{COOKIE_NAME, random_token, session_ttl};
use crate::db::{sessions, users};
use crate::error::{AppError, AppResult};
use crate::models::User;
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/sign-in", post(sign_in))
        .route("/complete-new-password", post(complete_new_password))
        .route("/forgot-password", post(forgot_password))
        .route("/confirm-forgot-password", post(confirm_forgot_password))
        .route("/me", get(me))
        .route("/logout", post(logout))
}

// ===== sign-in =====

#[derive(Deserialize)]
pub struct SignInInput {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum SignInResponse {
    Authenticated { user: User },
    NewPasswordRequired { session: String, email: String },
}

pub async fn sign_in(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(input): Json<SignInInput>,
) -> AppResult<Response> {
    let email = input.email.trim().to_lowercase();
    if email.is_empty() || input.password.is_empty() {
        return Err(AppError::Validation("email and password required".into()));
    }

    match state.cognito.sign_in(&email, &input.password).await? {
        SignInOutcome::Tokens(tokens) => {
            let (jar, user) = open_session(&state, jar, tokens).await?;
            Ok((jar, Json(SignInResponse::Authenticated { user })).into_response())
        }
        SignInOutcome::NewPasswordRequired { session } => Ok(Json(
            SignInResponse::NewPasswordRequired { session, email },
        )
        .into_response()),
    }
}

// ===== complete-new-password =====

#[derive(Deserialize)]
pub struct CompleteNewPasswordInput {
    pub email: String,
    pub new_password: String,
    pub session: String,
}

pub async fn complete_new_password(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(input): Json<CompleteNewPasswordInput>,
) -> AppResult<Response> {
    let email = input.email.trim().to_lowercase();
    if email.is_empty() || input.new_password.is_empty() || input.session.is_empty() {
        return Err(AppError::Validation(
            "email, new_password and session required".into(),
        ));
    }

    let tokens = state
        .cognito
        .complete_new_password(&email, &input.new_password, &input.session)
        .await?;
    let (jar, user) = open_session(&state, jar, tokens).await?;
    Ok((jar, Json(SignInResponse::Authenticated { user })).into_response())
}

// ===== forgot-password =====

#[derive(Deserialize)]
pub struct ForgotPasswordInput {
    pub email: String,
}

pub async fn forgot_password(
    State(state): State<AppState>,
    Json(input): Json<ForgotPasswordInput>,
) -> AppResult<StatusCode> {
    let email = input.email.trim().to_lowercase();
    if email.is_empty() {
        return Err(AppError::Validation("E-Mail erforderlich".into()));
    }

    state.cognito.forgot_password(&email).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct ConfirmForgotPasswordInput {
    pub email: String,
    pub code: String,
    pub new_password: String,
}

pub async fn confirm_forgot_password(
    State(state): State<AppState>,
    Json(input): Json<ConfirmForgotPasswordInput>,
) -> AppResult<StatusCode> {
    let email = input.email.trim().to_lowercase();
    let code = input.code.trim();
    if email.is_empty() || code.is_empty() || input.new_password.is_empty() {
        return Err(AppError::Validation(
            "E-Mail, Code und neues Passwort erforderlich".into(),
        ));
    }

    state
        .cognito
        .confirm_forgot_password(&email, code, &input.new_password)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ===== me =====

#[derive(Serialize)]
pub struct MeResponse {
    pub user: User,
}

pub async fn me(auth: AuthenticatedUser) -> Json<MeResponse> {
    Json(MeResponse { user: auth.user })
}

// ===== logout =====

pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> AppResult<Response> {
    if let Some(cookie) = jar.get(COOKIE_NAME) {
        let session_id = cookie.value().to_string();
        sessions::delete(&state.db, &session_id).await?;
    }

    let mut clear = Cookie::from(COOKIE_NAME);
    clear.set_path("/");
    let jar = jar.remove(clear);

    Ok((jar, axum::response::Redirect::to("/login")).into_response())
}

// ===== shared: token validation + session creation =====

async fn open_session(
    state: &AppState,
    jar: CookieJar,
    tokens: TokenSet,
) -> AppResult<(CookieJar, User)> {
    let claims = jwt::validate_id_token(
        &tokens.id_token,
        &state.jwks,
        &state.config.cognito_issuer_url(),
        &state.cognito.client_id,
    )
    .await?;

    let display_name = claims
        .name
        .clone()
        .or_else(|| claims.email.clone())
        .unwrap_or_else(|| claims.sub.clone());
    let email = claims.email.clone().unwrap_or_default();

    let user = users::upsert_from_claims(&state.db, &claims.sub, &email, &display_name).await?;

    let session_id = random_token(24);
    let ttl = session_ttl();
    sessions::put(
        &state.db,
        &sessions::Session {
            session_id: session_id.clone(),
            sub: claims.sub,
            refresh_token: tokens.refresh_token,
            created_at: Utc::now(),
            ttl,
        },
    )
    .await?;

    let cookie = build_session_cookie(state, session_id);
    Ok((jar.add(cookie), user))
}

fn build_session_cookie(state: &AppState, session_id: String) -> Cookie<'static> {
    let secure = state.config.env == "prod";

    Cookie::build((COOKIE_NAME, session_id))
        .path("/")
        .http_only(true)
        .secure(secure)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(30))
        .build()
}
