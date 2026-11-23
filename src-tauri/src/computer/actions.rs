use crate::error::{AppError, AppResult};
use serde::Deserialize;

use super::ComputerCapabilities;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ComputerAction {
    Click {
        x: i64,
        y: i64,
        #[serde(default)]
        button: Option<String>,
    },
    Move {
        x: i64,
        y: i64,
    },
    Type {
        text: String,
    },
    Key {
        key: String,
    },
    Scroll {
        amount: i64,
    },
}

pub(super) fn build_action(
    capabilities: &ComputerCapabilities,
    tool: &str,
    action: &ComputerAction,
) -> AppResult<(String, Vec<String>, serde_json::Value)> {
    let wayland = capabilities.backend == "wayland";
    match action {
        ComputerAction::Click { x, y, button } => {
            let button = match button.as_deref().unwrap_or("left") {
                "right" => "3",
                "middle" => "2",
                _ => "1",
            };
            if !(0..=20_000).contains(x) || !(0..=20_000).contains(y) {
                return Err(AppError::BadRequest("coordenadas fora do limite".into()));
            }
            if wayland {
                Ok((
                    tool.into(),
                    vec![
                        "mousemove".into(),
                        "-a".into(),
                        x.to_string(),
                        y.to_string(),
                    ],
                    serde_json::json!({ "type": "click", "x": x, "y": y }),
                ))
            } else {
                Ok((
                    tool.into(),
                    vec![
                        "mousemove".into(),
                        x.to_string(),
                        y.to_string(),
                        "click".into(),
                        button.into(),
                    ],
                    serde_json::json!({ "type": "click", "x": x, "y": y }),
                ))
            }
        }
        ComputerAction::Move { x, y } => {
            if !(0..=20_000).contains(x) || !(0..=20_000).contains(y) {
                return Err(AppError::BadRequest("coordenadas fora do limite".into()));
            }
            let args = if wayland {
                vec![
                    "mousemove".into(),
                    "-a".into(),
                    x.to_string(),
                    y.to_string(),
                ]
            } else {
                vec!["mousemove".into(), x.to_string(), y.to_string()]
            };
            Ok((
                tool.into(),
                args,
                serde_json::json!({ "type": "move", "x": x, "y": y }),
            ))
        }
        ComputerAction::Type { text } => {
            let text = text.trim_end_matches('\0');
            if text.len() > 4000 {
                return Err(AppError::BadRequest("texto grande demais".into()));
            }
            let args = if wayland {
                vec!["type".into(), "--".into(), text.to_string()]
            } else {
                vec![
                    "type".into(),
                    "--delay".into(),
                    "20".into(),
                    "--".into(),
                    text.to_string(),
                ]
            };
            // Never persist what was typed.
            Ok((
                tool.into(),
                args,
                serde_json::json!({ "type": "type", "length": text.chars().count() }),
            ))
        }
        ComputerAction::Key { key } => {
            let key = key.trim();
            if key.is_empty() || key.len() > 80 {
                return Err(AppError::BadRequest("tecla inválida".into()));
            }
            Ok((
                tool.into(),
                vec!["key".into(), key.to_string()],
                serde_json::json!({ "type": "key", "key": key }),
            ))
        }
        ComputerAction::Scroll { amount } => {
            let amount = (*amount).clamp(-20, 20);
            let args = if wayland {
                vec![
                    "click".into(),
                    if amount < 0 { "0x40" } else { "0x41" }.into(),
                ]
            } else {
                vec!["click".into(), "4".into()]
            };
            Ok((
                tool.into(),
                args,
                serde_json::json!({ "type": "scroll", "amount": amount }),
            ))
        }
    }
}
