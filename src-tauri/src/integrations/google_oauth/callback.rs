use axum::{
    Router,
    extract::{Query, State},
    response::Html,
    routing::get,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

impl super::GoogleOAuth {
    pub(super) async fn ensure_callback_server(self: &Arc<Self>) -> crate::error::AppResult<()> {
        let already_running = {
            let task = self
                .callback_task
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            task.as_ref()
                .map(|handle| !handle.is_finished())
                .unwrap_or(false)
        };
        if already_running {
            return Ok(());
        }

        let listener = TcpListener::bind(("127.0.0.1", self.callback_port))
            .await
            .map_err(|error| {
                crate::error::AppError::Daemon(format!(
                    "could not bind Google OAuth callback on 127.0.0.1:{}: {error}",
                    self.callback_port
                ))
            })?;
        let app = Router::new()
            .route("/callback", get(callback))
            .with_state(Arc::clone(self));

        let handle = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, app).await {
                tracing::warn!(error = %error, "Google OAuth callback server stopped");
            }
        });
        *self
            .callback_task
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(handle);
        Ok(())
    }
}

async fn callback(
    State(oauth): State<Arc<super::GoogleOAuth>>,
    Query(query): Query<CallbackQuery>,
) -> Html<String> {
    if let Some(error) = query.error {
        return Html(page(
            "Conexão cancelada",
            &format!("O Google retornou: {error}"),
        ));
    }
    let Some(state) = query.state else {
        return Html(page("Requisição inválida", "state ausente."));
    };
    let Some(code) = query.code else {
        return Html(page("Requisição inválida", "code ausente."));
    };
    let flow = oauth
        .pending
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .remove(&state);
    let Some(flow) = flow else {
        return Html(page(
            "Sessão expirada",
            "O state não foi encontrado. Inicie a conexão novamente.",
        ));
    };
    if Utc::now() - flow.created_at > Duration::minutes(15) {
        return Html(page(
            "Sessão expirada",
            "A conexão demorou mais de 15 minutos. Inicie novamente.",
        ));
    }

    match oauth.exchange_code(&flow, &code).await {
        Ok(()) => Html(page(
            "Google conectado",
            "Você pode fechar esta aba e voltar ao OpenDesk.",
        )),
        Err(error) => Html(page("Falha ao conectar", &format!("{error}"))),
    }
}

fn page(title: &str, message: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>body{{background:#09090b;color:#e4e4e7;font-family:Inter,Arial,sans-serif;\
         display:flex;align-items:center;justify-content:center;height:100vh;margin:0}}\
         .card{{border:1px solid #27272a;border-radius:12px;padding:24px;max-width:520px}}\
         h1{{font-size:18px;margin:0 0 8px}}p{{color:#a1a1aa;font-size:13px;line-height:1.6}}</style>\
         </head><body><div class=\"card\"><h1>{title}</h1><p>{message}</p></div></body></html>",
        title = html_escape::encode_text(title),
        message = html_escape::encode_text(message)
    )
}
