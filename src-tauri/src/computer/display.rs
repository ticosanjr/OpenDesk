use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ComputerCapabilities {
    pub available: bool,
    pub backend: String,
    pub display: Option<String>,
    pub screenshot_tool: Option<String>,
    pub input_tool: Option<String>,
    pub reason: Option<String>,
}

pub fn capabilities() -> ComputerCapabilities {
    capabilities_for(None)
}

/// Capabilities for a new session, including the Xvfb requirement when the
/// session should not touch the user's desktop.
pub fn can_start(isolated: bool) -> ComputerCapabilities {
    if !isolated {
        return capabilities();
    }
    let mut capabilities = capabilities_for(Some(":virtual"));
    if !crate::virtual_display::available() {
        capabilities.available = false;
        capabilities.reason =
            Some("display isolado exige Xvfb, xdotool e imagemagick (import)".into());
    }
    capabilities
}

pub fn capabilities_for(forced_display: Option<&str>) -> ComputerCapabilities {
    if let Some(display) = forced_display {
        let screenshot = ["import", "scrot"]
            .into_iter()
            .find(|command| command_exists(command));
        let input = ["xdotool", "ydotool"]
            .into_iter()
            .find(|command| command_exists(command));
        return ComputerCapabilities {
            available: screenshot.is_some() && input.is_some(),
            backend: "x11-isolated".into(),
            display: Some(display.to_string()),
            screenshot_tool: screenshot.map(str::to_string),
            input_tool: input.map(str::to_string),
            reason: if screenshot.is_none() {
                Some("instale imagemagick (import) para capturas no display virtual".into())
            } else if input.is_none() {
                Some("instale xdotool para entrada no display virtual".into())
            } else {
                None
            },
        };
    }
    let display = std::env::var("WAYLAND_DISPLAY")
        .ok()
        .map(|value| format!("wayland:{value}"))
        .or_else(|| {
            std::env::var("DISPLAY")
                .ok()
                .map(|value| format!("x11:{value}"))
        });
    let screenshot = ["grim", "import", "scrot"]
        .into_iter()
        .find(|command| command_exists(command));
    let input = ["ydotool", "xdotool"]
        .into_iter()
        .find(|command| command_exists(command));
    let backend = if display
        .as_deref()
        .is_some_and(|value| value.starts_with("wayland"))
    {
        "wayland"
    } else if display.is_some() {
        "x11"
    } else {
        "none"
    };
    let available = display.is_some() && screenshot.is_some() && input.is_some();
    let reason = if display.is_none() {
        Some("sessão gráfica não detectada (DISPLAY/WAYLAND_DISPLAY ausente)".into())
    } else if screenshot.is_none() {
        Some("instale grim (Wayland) ou imagemagick/scrot (X11) para capturas".into())
    } else if input.is_none() {
        Some("instale ydotool (Wayland) ou xdotool (X11) para entrada".into())
    } else {
        None
    };
    ComputerCapabilities {
        available,
        backend: backend.into(),
        display,
        screenshot_tool: screenshot.map(str::to_string),
        input_tool: input.map(str::to_string),
        reason,
    }
}

fn command_exists(command: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(command).is_file()))
        .unwrap_or(false)
}
