// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::auth::AuthenticatedUser;
use crate::auth::jwt;
use crate::auth::session::{COOKIE_NAME, pkce_pair, random_token, session_ttl};
use crate::db::{oauth_state, sessions, users};
use crate::error::{AppError, AppResult};
use crate::models::User;
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/login", get(login))
        .route("/callback", get(callback))
        .route("/me", get(me))
        .route("/logout", post(logout))
}

#[derive(Deserialize)]
pub struct LoginQuery {
    /// Where to send the user after successful login. Defaults to
    /// `FRONTEND_BASE_URL`. The value must be parseable as a URL but is not
    /// otherwise validated — only the configured frontend should ever issue
    /// these requests.
    pub redirect: Option<String>,
}

pub async fn login(
    State(state): State<AppState>,
    Query(q): Query<LoginQuery>,
) -> AppResult<Response> {
    let oauth_state_token = random_token(24);
    let (verifier, challenge) = pkce_pair();

    let redirect_after = q
        .redirect
        .unwrap_or_else(|| state.config.frontend_base_url.clone());

    oauth_state::put(&state.db, &oauth_state_token, &verifier, &redirect_after).await?;

    let mut url = Url::parse(&format!(
        "https://{}/oauth2/authorize",
        state.cognito.hosted_ui_domain
    ))
    .map_err(|e| AppError::Internal(format!("authorize url: {e}")))?;

    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &state.cognito.client_id)
        .append_pair("redirect_uri", &state.cognito.redirect_uri)
        .append_pair("scope", "openid email profile")
        .append_pair("state", &oauth_state_token)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");

    Ok(Redirect::to(url.as_str()).into_response())
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

pub async fn callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<CallbackQuery>,
) -> AppResult<Response> {
    if let Some(err) = &q.error {
        tracing::warn!(
            error = %err,
            description = ?q.error_description,
            "Cognito callback returned an error"
        );
        return Err(AppError::Unauthorized);
    }

    let code = q.code.ok_or(AppError::Unauthorized)?;
    let state_token = q.state.ok_or(AppError::Unauthorized)?;

    let oauth = oauth_state::take(&state.db, &state_token)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let tokens = state
        .cognito
        .exchange_code(&code, &oauth.code_verifier)
        .await?;

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

    users::upsert_from_claims(&state.db, &claims.sub, &email, &display_name).await?;

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

    let cookie = build_session_cookie(&state, session_id);
    let jar = jar.add(cookie);

    Ok((jar, Redirect::to(&oauth.redirect_after)).into_response())
}

#[derive(Serialize)]
pub struct MeResponse {
    pub user: User,
}

pub async fn me(auth: AuthenticatedUser) -> Json<MeResponse> {
    Json(MeResponse { user: auth.user })
}

pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> AppResult<Response> {
    if let Some(cookie) = jar.get(COOKIE_NAME) {
        let session_id = cookie.value().to_string();
        if let Some(session) = sessions::get(&state.db, &session_id).await? {
            let _ = state.cognito.revoke_refresh_token(&session.refresh_token).await;
        }
        sessions::delete(&state.db, &session_id).await?;
    }

    let mut clear = Cookie::from(COOKIE_NAME);
    clear.set_path("/");
    let jar = jar.remove(clear);

    let mut url = Url::parse(&format!(
        "https://{}/logout",
        state.cognito.hosted_ui_domain
    ))
    .map_err(|e| AppError::Internal(format!("logout url: {e}")))?;
    url.query_pairs_mut()
        .append_pair("client_id", &state.cognito.client_id)
        .append_pair("logout_uri", &state.config.frontend_base_url);

    Ok((jar, Redirect::to(url.as_str())).into_response())
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
