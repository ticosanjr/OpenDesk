use super::google_oauth::GoogleOAuth;
use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use lettre::message::{Attachment, Message, MultiPart, SinglePart, header::ContentType};
use pulldown_cmark::{Options, Parser, html};
use serde_json::{Value, json};
use std::sync::Arc;

pub struct GoogleApi {
    oauth: Arc<GoogleOAuth>,
}

impl GoogleApi {
    pub fn new(oauth: Arc<GoogleOAuth>) -> Self {
        Self { oauth }
    }

    async fn token(&self, workspace_id: &str) -> Result<String> {
        Ok(self.oauth.access_token(workspace_id).await?)
    }

    pub async fn drive_create_folder(
        &self,
        workspace_id: &str,
        name: &str,
        parent_id: Option<&str>,
    ) -> Result<Value> {
        let mut metadata = json!({
            "name": name,
            "mimeType": "application/vnd.google-apps.folder",
        });
        if let Some(parent) = parent_id {
            metadata["parents"] = json!([parent]);
        }
        self.post_json(
            workspace_id,
            "https://www.googleapis.com/drive/v3/files?fields=id,webViewLink",
            metadata,
        )
        .await
    }

    pub async fn drive_search(&self, workspace_id: &str, query: &str) -> Result<Value> {
        let token = self.token(workspace_id).await?;
        let response = self
            .oauth
            .http()
            .get("https://www.googleapis.com/drive/v3/files")
            .bearer_auth(token)
            .query(&[
                ("q", query.to_string()),
                (
                    "fields",
                    "files(id,name,mimeType,webViewLink,modifiedTime)".into(),
                ),
                ("pageSize", "50".into()),
            ])
            .send()
            .await?;
        parse_json(response).await
    }

    pub async fn drive_upload(
        &self,
        workspace_id: &str,
        name: &str,
        mime: &str,
        bytes: Vec<u8>,
        parent_id: Option<&str>,
        convert_to_doc: bool,
    ) -> Result<Value> {
        let token = self.token(workspace_id).await?;
        let mut metadata = json!({
            "name": name,
            "mimeType": if convert_to_doc { "application/vnd.google-apps.document" } else { mime },
        });
        if let Some(parent) = parent_id {
            metadata["parents"] = json!([parent]);
        }

        let form = reqwest::multipart::Form::new()
            .text("metadata", serde_json::to_string(&metadata)?)
            .part(
                "file",
                reqwest::multipart::Part::bytes(bytes)
                    .file_name(name.to_string())
                    .mime_str(mime)?,
            );

        let response = self
            .oauth
            .http()
            .post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&fields=id,webViewLink")
            .bearer_auth(token)
            .multipart(form)
            .send()
            .await?;
        parse_json(response).await
    }

    pub async fn gdocs_create_from_markdown(
        &self,
        workspace_id: &str,
        title: &str,
        markdown: &str,
        folder_id: Option<&str>,
    ) -> Result<Value> {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(markdown, options);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);
        let document =
            format!("<html><head><meta charset=\"utf-8\"></head><body>{html_output}</body></html>");
        self.drive_upload(
            workspace_id,
            title,
            "text/html",
            document.into_bytes(),
            folder_id,
            true,
        )
        .await
    }

    pub async fn gdocs_read(&self, workspace_id: &str, document_id: &str) -> Result<String> {
        let token = self.token(workspace_id).await?;
        let response = self
            .oauth
            .http()
            .get(format!(
                "https://docs.googleapis.com/v1/documents/{document_id}"
            ))
            .bearer_auth(token)
            .send()
            .await?;
        let document: Value = parse_json(response).await?;
        Ok(extract_doc_text(&document))
    }

    pub async fn gdocs_append_text(
        &self,
        workspace_id: &str,
        document_id: &str,
        text: &str,
    ) -> Result<Value> {
        let token = self.token(workspace_id).await?;
        let response = self
            .oauth
            .http()
            .get(format!(
                "https://docs.googleapis.com/v1/documents/{document_id}"
            ))
            .bearer_auth(&token)
            .send()
            .await?;
        let document: Value = parse_json(response).await?;
        let end_index = document
            .pointer("/body/content")
            .and_then(Value::as_array)
            .and_then(|content| content.last())
            .and_then(|node| node.get("endIndex"))
            .and_then(Value::as_u64)
            .unwrap_or(2);
        self.post_json(
            workspace_id,
            &format!("https://docs.googleapis.com/v1/documents/{document_id}:batchUpdate"),
            json!({
                "requests": [{
                    "insertText": {
                        "location": { "index": end_index.saturating_sub(1) },
                        "text": text
                    }
                }]
            }),
        )
        .await
    }

    pub async fn gmail_send(
        &self,
        workspace_id: &str,
        to: &str,
        subject: &str,
        html_body: &str,
        attachments: Vec<(String, Vec<u8>, String)>,
    ) -> Result<Value> {
        let from = self
            .oauth
            .status(workspace_id)
            .await
            .ok()
            .and_then(|status| status.email)
            .unwrap_or_else(|| "me@localhost".to_string());
        let raw = build_gmail_raw(&from, to, subject, html_body, attachments)?;
        self.post_json(
            workspace_id,
            "https://gmail.googleapis.com/gmail/v1/users/me/messages/send",
            json!({ "raw": raw }),
        )
        .await
    }

    pub async fn gmail_create_draft(
        &self,
        workspace_id: &str,
        to: &str,
        subject: &str,
        html_body: &str,
    ) -> Result<Value> {
        let from = self
            .oauth
            .status(workspace_id)
            .await
            .ok()
            .and_then(|status| status.email)
            .unwrap_or_else(|| "me@localhost".to_string());
        let raw = build_gmail_raw(&from, to, subject, html_body, Vec::new())?;
        self.post_json(
            workspace_id,
            "https://gmail.googleapis.com/gmail/v1/users/me/drafts",
            json!({ "message": { "raw": raw } }),
        )
        .await
    }

    pub async fn gmail_search(&self, workspace_id: &str, query: &str) -> Result<Value> {
        let token = self.token(workspace_id).await?;
        let response = self
            .oauth
            .http()
            .get("https://gmail.googleapis.com/gmail/v1/users/me/threads")
            .bearer_auth(token)
            .query(&[("q", query), ("maxResults", "20")])
            .send()
            .await?;
        parse_json(response).await
    }

    pub async fn calendar_list(
        &self,
        workspace_id: &str,
        time_min: Option<&str>,
        time_max: Option<&str>,
    ) -> Result<Value> {
        let token = self.token(workspace_id).await?;
        let mut request = self
            .oauth
            .http()
            .get("https://www.googleapis.com/calendar/v3/calendars/primary/events")
            .bearer_auth(token)
            .query(&[
                ("singleEvents", "true"),
                ("orderBy", "startTime"),
                ("maxResults", "50"),
            ]);
        if let Some(time_min) = time_min {
            request = request.query(&[("timeMin", time_min)]);
        }
        if let Some(time_max) = time_max {
            request = request.query(&[("timeMax", time_max)]);
        }
        parse_json(request.send().await?).await
    }

    pub async fn calendar_create(
        &self,
        workspace_id: &str,
        summary: &str,
        description: Option<&str>,
        start: &str,
        end: &str,
        timezone: Option<&str>,
    ) -> Result<Value> {
        self.calendar_create_in(
            workspace_id,
            "primary",
            summary,
            description,
            None,
            start,
            end,
            timezone,
            false,
        )
        .await
    }

    pub async fn calendar_list_calendars(&self, workspace_id: &str) -> Result<Value> {
        let token = self.token(workspace_id).await?;
        let response = self
            .oauth
            .http()
            .get("https://www.googleapis.com/calendar/v3/users/me/calendarList")
            .bearer_auth(token)
            .query(&[("minAccessRole", "reader"), ("maxResults", "250")])
            .send()
            .await?;
        parse_json(response).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn calendar_events_page(
        &self,
        workspace_id: &str,
        calendar_id: &str,
        time_min: Option<&str>,
        time_max: Option<&str>,
        sync_token: Option<&str>,
        page_token: Option<&str>,
    ) -> Result<Value> {
        let token = self.token(workspace_id).await?;
        let url = format!(
            "https://www.googleapis.com/calendar/v3/calendars/{}/events",
            urlencoding::encode(calendar_id)
        );
        let mut request = self.oauth.http().get(url).bearer_auth(token).query(&[
            ("singleEvents", "true"),
            ("showDeleted", "true"),
            ("maxResults", "2500"),
        ]);
        if let Some(sync_token) = sync_token {
            request = request.query(&[("syncToken", sync_token)]);
        } else {
            if let Some(time_min) = time_min {
                request = request.query(&[("timeMin", time_min)]);
            }
            if let Some(time_max) = time_max {
                request = request.query(&[("timeMax", time_max)]);
            }
        }
        if let Some(page_token) = page_token {
            request = request.query(&[("pageToken", page_token)]);
        }
        parse_json(request.send().await?).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn calendar_create_in(
        &self,
        workspace_id: &str,
        calendar_id: &str,
        summary: &str,
        description: Option<&str>,
        location: Option<&str>,
        start: &str,
        end: &str,
        timezone: Option<&str>,
        all_day: bool,
    ) -> Result<Value> {
        let url = format!(
            "https://www.googleapis.com/calendar/v3/calendars/{}/events",
            urlencoding::encode(calendar_id)
        );
        self.post_json(
            workspace_id,
            &url,
            calendar_event_payload(
                summary,
                description,
                location,
                start,
                end,
                timezone,
                all_day,
            ),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn calendar_patch(
        &self,
        workspace_id: &str,
        calendar_id: &str,
        event_id: &str,
        summary: &str,
        description: Option<&str>,
        location: Option<&str>,
        start: &str,
        end: &str,
        timezone: Option<&str>,
        all_day: bool,
    ) -> Result<Value> {
        let token = self.token(workspace_id).await?;
        let url = format!(
            "https://www.googleapis.com/calendar/v3/calendars/{}/events/{}",
            urlencoding::encode(calendar_id),
            urlencoding::encode(event_id)
        );
        let response = self
            .oauth
            .http()
            .patch(url)
            .bearer_auth(token)
            .json(&calendar_event_payload(
                summary,
                description,
                location,
                start,
                end,
                timezone,
                all_day,
            ))
            .send()
            .await?;
        parse_json(response).await
    }

    pub async fn calendar_delete(
        &self,
        workspace_id: &str,
        calendar_id: &str,
        event_id: &str,
    ) -> Result<()> {
        let token = self.token(workspace_id).await?;
        let url = format!(
            "https://www.googleapis.com/calendar/v3/calendars/{}/events/{}",
            urlencoding::encode(calendar_id),
            urlencoding::encode(event_id)
        );
        let response = self
            .oauth
            .http()
            .delete(url)
            .bearer_auth(token)
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Google Calendar returned {status}: {text}");
        }
        Ok(())
    }

    pub async fn tasks_list(&self, workspace_id: &str) -> Result<Value> {
        let token = self.token(workspace_id).await?;
        let lists = self
            .oauth
            .http()
            .get("https://tasks.googleapis.com/tasks/v1/users/@me/lists")
            .bearer_auth(&token)
            .send()
            .await?;
        let lists: Value = parse_json(lists).await?;
        let list_id = lists
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string);

        let Some(list_id) = list_id else {
            return Ok(json!({ "items": [] }));
        };
        let response = self
            .oauth
            .http()
            .get(format!(
                "https://tasks.googleapis.com/tasks/v1/lists/{list_id}/tasks"
            ))
            .bearer_auth(token)
            .send()
            .await?;
        parse_json(response).await
    }

    pub async fn tasks_create(
        &self,
        workspace_id: &str,
        title: &str,
        notes: Option<&str>,
    ) -> Result<Value> {
        let token = self.token(workspace_id).await?;
        let lists = self
            .oauth
            .http()
            .get("https://tasks.googleapis.com/tasks/v1/users/@me/lists")
            .bearer_auth(&token)
            .send()
            .await?;
        let lists: Value = parse_json(lists).await?;
        let list_id = lists
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string);

        let list_id = match list_id {
            Some(id) => id,
            None => {
                let created = self
                    .post_json(
                        workspace_id,
                        "https://tasks.googleapis.com/tasks/v1/users/@me/lists",
                        json!({ "title": "OpenDesk" }),
                    )
                    .await?;
                created
                    .get("id")
                    .and_then(Value::as_str)
                    .context("Google Tasks did not return a list id")?
                    .to_string()
            }
        };

        self.post_json(
            workspace_id,
            &format!("https://tasks.googleapis.com/tasks/v1/lists/{list_id}/tasks"),
            json!({ "title": title, "notes": notes }),
        )
        .await
    }

    async fn post_json(&self, workspace_id: &str, url: &str, body: Value) -> Result<Value> {
        let token = self.token(workspace_id).await?;
        let response = self
            .oauth
            .http()
            .post(url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await?;
        parse_json(response).await
    }
}

fn calendar_event_payload(
    summary: &str,
    description: Option<&str>,
    location: Option<&str>,
    start: &str,
    end: &str,
    timezone: Option<&str>,
    all_day: bool,
) -> Value {
    let timezone = timezone.unwrap_or("UTC");
    let (start, end) = if all_day {
        (
            json!({ "date": start.get(..10).unwrap_or(start) }),
            json!({ "date": end.get(..10).unwrap_or(end) }),
        )
    } else {
        (
            json!({ "dateTime": start, "timeZone": timezone }),
            json!({ "dateTime": end, "timeZone": timezone }),
        )
    };
    json!({
        "summary": summary,
        "description": description,
        "location": location,
        "start": start,
        "end": end,
    })
}

async fn parse_json(response: reqwest::Response) -> Result<Value> {
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        anyhow::bail!("Google API returned {status}: {text}");
    }
    if text.trim().is_empty() {
        Ok(Value::Null)
    } else {
        serde_json::from_str(&text).context("invalid Google API JSON")
    }
}

fn build_gmail_raw(
    from: &str,
    to: &str,
    subject: &str,
    html_body: &str,
    attachments: Vec<(String, Vec<u8>, String)>,
) -> Result<String> {
    let mut body = MultiPart::mixed()
        .multipart(MultiPart::alternative().singlepart(SinglePart::html(html_body.to_string())));
    for (filename, bytes, mime) in attachments {
        let content_type = ContentType::parse(&mime).unwrap_or_else(|_| {
            ContentType::parse("application/octet-stream").unwrap_or(ContentType::TEXT_PLAIN)
        });
        body = body.singlepart(Attachment::new(filename).body(bytes, content_type));
    }

    let email = Message::builder()
        .from(from.parse().context("invalid Gmail sender")?)
        .to(to.parse().context("invalid Gmail recipient")?)
        .subject(subject)
        .multipart(body)
        .context("could not build Gmail message")?;
    Ok(URL_SAFE_NO_PAD.encode(email.formatted()))
}

fn extract_doc_text(node: &Value) -> String {
    let mut output = String::new();
    if let Some(text) = node.pointer("/textRun/content").and_then(Value::as_str) {
        output.push_str(text);
    }
    if let Some(paragraph) = node.pointer("/paragraph")
        && let Some(elements) = paragraph.get("elements").and_then(Value::as_array)
    {
        for element in elements {
            output.push_str(&extract_doc_text(element));
        }
    }
    if let Some(content) = node.get("content").and_then(Value::as_array) {
        for child in content {
            output.push_str(&extract_doc_text(child));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use serde_json::json;

    #[test]
    fn timed_and_all_day_calendar_payloads_match_google_contract() {
        let timed = calendar_event_payload(
            "Planejamento",
            Some("Sala 1"),
            Some("Sala 2"),
            "2026-11-10T12:00:00Z",
            "2026-11-10T13:00:00Z",
            Some("America/Sao_Paulo"),
            false,
        );
        assert_eq!(timed["start"]["dateTime"], "2026-11-10T12:00:00Z");
        assert_eq!(timed["start"]["timeZone"], "America/Sao_Paulo");
        assert!(timed["start"].get("date").is_none());

        let all_day = calendar_event_payload(
            "Feriado",
            None,
            None,
            "2026-11-10T00:00:00Z",
            "2026-11-11T00:00:00Z",
            None,
            true,
        );
        assert_eq!(all_day["start"]["date"], "2026-11-10");
        assert_eq!(all_day["end"]["date"], "2026-11-11");
        assert!(all_day["start"].get("dateTime").is_none());
    }

    #[test]
    fn builds_a_decodable_gmail_raw_message() {
        let raw = build_gmail_raw(
            "sender@example.com",
            "recipient@example.com",
            "Resumo",
            "<p>ok</p>",
            vec![(
                "report.txt".into(),
                b"conteudo".to_vec(),
                "text/plain".into(),
            )],
        )
        .expect("raw");
        let decoded =
            String::from_utf8(URL_SAFE_NO_PAD.decode(raw).expect("base64")).expect("utf8");
        assert!(decoded.contains("From: sender@example.com"));
        assert!(decoded.contains("To: recipient@example.com"));
        assert!(decoded.contains("Subject: Resumo"));
        assert!(decoded.contains("<p>ok</p>"));
        assert!(decoded.contains("report.txt"));
    }

    #[test]
    fn extracts_nested_document_text() {
        let document = json!({
            "content": [{
                "paragraph": {
                    "elements": [
                        { "textRun": { "content": "Olá " } },
                        { "textRun": { "content": "mundo" } }
                    ]
                }
            }]
        });
        assert_eq!(extract_doc_text(&document), "Olá mundo");
    }
}
