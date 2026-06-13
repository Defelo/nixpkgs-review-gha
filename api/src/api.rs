use std::{fmt::Display, sync::Arc};

use anyhow::{Context, anyhow};
use axum::{
    Json, Router,
    response::{IntoResponse, Redirect, Response},
    routing,
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use listenfd::ListenFd;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tokio::net::UnixListener;
use tracing::debug;

use crate::{
    github::{approve_nixpkgs_pr, oidc::IdTokenVerifier, post_nixpkgs_comment},
    nixpkgs_review::{Report, ReportMarkdownRenderer},
};

pub struct State {
    pub oidc_client_id: String,
    pub id_token_verifier: IdTokenVerifier,
    pub report_markdown_renderer: ReportMarkdownRenderer,
    pub github_token: String,
}

pub async fn serve(state: State) -> anyhow::Result<()> {
    let mut listenfd = ListenFd::from_env();
    let listener = listenfd
        .take_unix_listener(0)?
        .ok_or_else(|| anyhow!("Expected to be passed a unix socket"))?;
    listener.set_nonblocking(true)?;
    let listener = UnixListener::from_std(listener)?;

    let router = Router::new()
        .route("/", routing::get(index))
        .route("/oidc_client_id", routing::get(get_client_id))
        .route("/submit_report", routing::post(submit_report))
        .with_state(Arc::new(state));

    axum::serve(listener, router.into_make_service()).await?;

    Ok(())
}

async fn index() -> Redirect {
    Redirect::temporary("https://github.com/Defelo/nixpkgs-review-gha")
}

async fn get_client_id(state: axum::extract::State<Arc<State>>) -> String {
    state.oidc_client_id.clone()
}

#[derive(Debug, Deserialize)]
struct SubmitReportRequest {
    #[serde(default = "default_post_result")]
    post_result: bool,
    #[serde(default)]
    on_success: OnSuccessAction,
    #[serde(flatten)]
    report: Report,
}

fn default_post_result() -> bool {
    true
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnSuccessAction {
    #[default]
    Nothing,
    MarkAsReady,
    Approve,
    Merge,
}

#[derive(Debug, Default, Serialize)]
struct SubmitReportResponse {
    comment_url: Option<String>,
    approval_url: Option<String>,
    errors: Vec<&'static str>,
}

async fn submit_report(
    state: axum::extract::State<Arc<State>>,
    auth: TypedHeader<Authorization<Bearer>>,
    Json(SubmitReportRequest {
        post_result,
        on_success,
        report,
    }): Json<SubmitReportRequest>,
) -> Result<Response, Response> {
    let claims = state
        .id_token_verifier
        .verify(auth.token())
        .map_err(|err| (StatusCode::UNAUTHORIZED, format!("invalid token: {err}")).into_response())
        .inspect_err(|_| debug!(token = auth.token(), "unauthorized report submit request"))?;

    debug!(?claims, post_result, ?on_success, "report submitted");

    let mut response = SubmitReportResponse::default();

    if post_result {
        let rendered = state.report_markdown_renderer.render(&report, &claims);

        let comment_url = post_nixpkgs_comment(&state.github_token, report.pr, &rendered)
            .await
            .with_context(|| format!("failed to post comment on pr #{}", report.pr))
            .map_err(internal_server_error)?;

        debug!(comment_url, "comment created");
        response.comment_url = Some(comment_url);
    }

    match on_success {
        _ if !report.is_success() => {}
        OnSuccessAction::Nothing => {}
        OnSuccessAction::MarkAsReady => response
            .errors
            .push("cannot mark PRs as ready for review yet"),
        OnSuccessAction::Approve => {
            let body = format!(
                "Approved automatically on behalf of @{} ({}) following the successful run of \
                 `nixpkgs-review` ([Logs]({}))",
                claims.actor,
                claims.actor_id,
                claims.logs_url()
            );

            let approval_url =
                approve_nixpkgs_pr(&state.github_token, report.pr, &report.head, &body)
                    .await
                    .with_context(|| {
                        format!(
                            "failed to approve pr #{} at commit {}",
                            report.pr, report.head
                        )
                    })
                    .map_err(internal_server_error)?;

            debug!(approval_url, "pr approved");
            response.approval_url = Some(approval_url);
        }
        OnSuccessAction::Merge => response.errors.push("cannot merge PRs yet"),
    }

    Ok(Json(response).into_response())
}

fn internal_server_error(err: impl Display) -> Response {
    tracing::error!("{err}");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
}
