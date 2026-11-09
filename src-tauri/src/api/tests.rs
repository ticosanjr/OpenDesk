use super::*;
use crate::db::{artifacts, calendar as calendar_db, deliveries, queue as queue_db, runs};
use crate::test_support::{self, TestContext};
use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header::AUTHORIZATION},
};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn call(
    ctx: &TestContext,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(AUTHORIZATION, format!("Bearer {}", ctx.state.api_token));
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let request = builder
        .body(
            body.map(|value| Body::from(value.to_string()))
                .unwrap_or_else(Body::empty),
        )
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).to_string()))
    };
    (status, value)
}

async fn unauthenticated(ctx: &TestContext, uri: &str) -> StatusCode {
    let request = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request");
    router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response")
        .status()
}

mod attention;
mod audit;
mod backups;
mod browse;
mod calendar;
mod core;
mod export;
mod governance;
mod io_bus;
mod learning;
mod metrics;
mod resources;
mod schedules;
mod studio;
mod tasks;
