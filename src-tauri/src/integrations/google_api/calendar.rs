use super::GoogleApi;
use super::client::parse_json;
use anyhow::{Context, Result};
use serde_json::{Value, json};

impl GoogleApi {
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
}

pub(super) fn calendar_event_payload(
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
