use crate::{
    db::io_bus::OutboundSink,
    error::{AppError, AppResult},
    io_bus::{IoContext, RunOutput, config_value, outbound::artifact_paths},
};
use s3::{Bucket, Region, creds::Credentials};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Config {
    bucket: String,
    #[serde(default)]
    region: Option<String>,
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    access_key_ref: Option<String>,
    #[serde(default)]
    secret_key_ref: Option<String>,
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    path_style: bool,
}

pub async fn run(
    ctx: &IoContext,
    sink: &OutboundSink,
    output: &RunOutput,
) -> AppResult<Vec<String>> {
    let config: Config = config_value(sink)?;
    let secrets = ctx.secrets();
    let access = config
        .access_key_ref
        .as_deref()
        .and_then(|key| secrets.get("io_bus", key).ok().flatten())
        .or_else(|| std::env::var("OPENDESK_S3_ACCESS_KEY").ok());
    let secret = config
        .secret_key_ref
        .as_deref()
        .and_then(|key| secrets.get("io_bus", key).ok().flatten())
        .or_else(|| std::env::var("OPENDESK_S3_SECRET_KEY").ok());
    let credentials = Credentials::new(access.as_deref(), secret.as_deref(), None, None, None)
        .map_err(|error| AppError::BadRequest(format!("credenciais S3: {error}")))?;
    let region = match config.endpoint.clone() {
        Some(endpoint) => Region::Custom {
            region: config.region.unwrap_or_else(|| "us-east-1".into()),
            endpoint,
        },
        None => Region::Custom {
            region: config.region.unwrap_or_else(|| "us-east-1".into()),
            endpoint: "https://s3.amazonaws.com".into(),
        },
    };
    let mut bucket = Bucket::new(&config.bucket, region, credentials)
        .map_err(|error| AppError::BadRequest(format!("bucket S3: {error}")))?;
    if config.path_style || config.endpoint.is_some() {
        bucket = bucket.with_path_style();
    }

    let mut uploaded = Vec::new();
    for path in artifact_paths(output) {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("artifact");
        let key = config
            .prefix
            .as_deref()
            .map(|prefix| format!("{}/{}", prefix.trim_end_matches('/'), name))
            .unwrap_or_else(|| name.to_string());
        let bytes = std::fs::read(path)?;
        let mime = crate::api::files::mime_for(Path::new(path));
        bucket
            .put_object_with_content_type(&key, &bytes, mime)
            .await
            .map_err(|error| AppError::Internal(format!("S3 upload: {error}")))?;
        uploaded.push(key);
    }
    if uploaded.is_empty() {
        return Err(AppError::BadRequest(
            "MINIO_S3 não encontrou artefatos para enviar".into(),
        ));
    }
    Ok(uploaded)
}
