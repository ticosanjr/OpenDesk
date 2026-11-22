use crate::{
    db::io_bus::OutboundSink,
    error::{AppError, AppResult},
    io_bus::{IoContext, RunOutput, config_value, outbound::artifact_paths, outbound::output_root},
};
use rust_xlsxwriter::{Format, Workbook};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
struct Config {
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    filename: Option<String>,
}

pub async fn run(
    _ctx: &IoContext,
    sink: &OutboundSink,
    output: &RunOutput,
) -> AppResult<Vec<String>> {
    let config: Config = config_value(sink)?;
    let root = output_root(output);
    let source = match config.source.as_deref() {
        Some(path) => {
            let path = Path::new(path);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                root.join(path)
            }
        }
        None => artifact_paths(output)
            .into_iter()
            .find(|path| {
                matches!(
                    path.extension().and_then(|value| value.to_str()),
                    Some("md" | "json" | "csv" | "typ")
                )
            })
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                AppError::BadRequest("DOC_COMPILER não encontrou .md/.json/.csv/.typ no run".into())
            })?,
    };
    if !source.exists() {
        return Err(AppError::NotFound(format!(
            "arquivo de origem {}",
            source.display()
        )));
    }
    let format = config
        .format
        .as_deref()
        .map(str::to_lowercase)
        .unwrap_or_else(|| {
            source
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("pdf")
                .to_lowercase()
        });
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("documento");
    let filename = config.filename.unwrap_or_else(|| match format.as_str() {
        "xlsx" => format!("{stem}.xlsx"),
        "csv" => format!("{stem}.csv"),
        _ => format!("{stem}.pdf"),
    });
    let destination = root.join(filename);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match format.as_str() {
        "xlsx" => compile_xlsx(&source, &destination)?,
        "csv" => {
            let source_text = std::fs::read_to_string(&source)?;
            std::fs::write(&destination, source_text)?;
        }
        "pdf" => compile_pdf(&source, &destination).await?,
        other => {
            return Err(AppError::BadRequest(format!(
                "formato de documento não suportado: {other}"
            )));
        }
    }
    Ok(vec![destination.to_string_lossy().to_string()])
}

fn compile_xlsx(source: &Path, destination: &Path) -> AppResult<()> {
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let rows: Vec<Vec<serde_json::Value>> = if extension == "csv" {
        parse_csv(&std::fs::read_to_string(source)?)
    } else {
        let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(source)?)
            .map_err(|error| AppError::BadRequest(format!("JSON inválido: {error}")))?;
        parse_json_rows(&value)
    };
    let mut workbook = Workbook::new();
    let header = Format::new().set_bold().set_background_color(0xF1F5F9);
    let sheet = workbook.add_worksheet();
    sheet.set_freeze_panes(1, 0).map_err(xlsx_error)?;
    let mut widths: Vec<usize> = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        for (column, value) in row.iter().enumerate() {
            let cell = cell_text(value);
            widths.resize(widths.len().max(column + 1), 8);
            widths[column] = widths[column].max(cell.chars().count().min(60) + 2);
            if row_index == 0 {
                sheet
                    .write_string_with_format(0, column as u16, &cell, &header)
                    .map_err(xlsx_error)?;
            } else if let Some(number) = value.as_f64() {
                sheet
                    .write_number(row_index as u32, column as u16, number)
                    .map_err(xlsx_error)?;
            } else {
                sheet
                    .write_string(row_index as u32, column as u16, &cell)
                    .map_err(xlsx_error)?;
            }
        }
    }
    for (column, width) in widths.iter().enumerate() {
        sheet
            .set_column_width(column as u16, *width as f64)
            .map_err(xlsx_error)?;
    }
    workbook
        .save(destination)
        .map_err(|error| AppError::Internal(format!("XLSX: {error}")))?;
    Ok(())
}

fn xlsx_error(error: rust_xlsxwriter::XlsxError) -> AppError {
    AppError::Internal(format!("XLSX: {error}"))
}

fn parse_csv(text: &str) -> Vec<Vec<serde_json::Value>> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split(',')
                .map(|value| serde_json::Value::String(value.trim().to_string()))
                .collect()
        })
        .collect()
}

fn parse_json_rows(value: &serde_json::Value) -> Vec<Vec<serde_json::Value>> {
    let array = match value {
        serde_json::Value::Array(items) => items.clone(),
        serde_json::Value::Object(object) => {
            if let Some(items) = object
                .values()
                .find_map(serde_json::Value::as_array)
                .cloned()
            {
                items
            } else {
                vec![serde_json::Value::Object(object.clone())]
            }
        }
        other => vec![other.clone()],
    };
    let mut rows = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    for item in &array {
        if let Some(object) = item.as_object() {
            for key in object.keys() {
                if !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
        }
    }
    if !keys.is_empty() {
        rows.push(
            keys.iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        );
    }
    for item in array {
        if let Some(object) = item.as_object() {
            rows.push(
                keys.iter()
                    .map(|key| object.get(key).cloned().unwrap_or(serde_json::Value::Null))
                    .collect(),
            );
        } else {
            rows.push(vec![item]);
        }
    }
    rows
}

fn cell_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

async fn compile_pdf(source: &Path, destination: &Path) -> AppResult<()> {
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let (typst_source, temporary) = if extension == "typ" {
        (source.to_path_buf(), None)
    } else if extension == "md" {
        let markdown = std::fs::read_to_string(source)?;
        let typst = markdown_to_typst(&markdown);
        let temporary = source.with_extension("opendesk.typ");
        std::fs::write(&temporary, typst)?;
        (temporary.clone(), Some(temporary))
    } else {
        return Err(AppError::BadRequest(
            "PDF só é gerado a partir de .typ ou .md".into(),
        ));
    };

    let mut command = tokio::process::Command::new(crate::io_bus::tools::command("typst"));
    command
        .args([
            "compile",
            &typst_source.to_string_lossy(),
            &destination.to_string_lossy(),
        ])
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let result = tokio::time::timeout(std::time::Duration::from_secs(120), command.output())
        .await
        .map_err(|_| AppError::Internal("typst timeout".into()))?
        .map_err(|error| {
            AppError::BadRequest(format!(
                "typst não encontrado; instale em Configurações → Ferramentas: {error}"
            ))
        })?;
    if let Some(path) = temporary {
        let _ = std::fs::remove_file(path);
    }
    if !result.status.success() {
        return Err(AppError::Internal(format!(
            "typst falhou: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        )));
    }
    Ok(())
}

/// Small, deterministic Markdown → Typst subset. Unsupported constructs are
/// rendered as plain text instead of silently producing a broken document.
pub fn markdown_to_typst(markdown: &str) -> String {
    let mut out = String::from("#set page(paper: \"a4\", margin: 2cm)\n#set text(size: 10pt)\n\n");
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push('\n');
        } else if let Some(heading) = trimmed.strip_prefix("### ") {
            out.push_str(&format!("=== {heading}\n"));
        } else if let Some(heading) = trimmed.strip_prefix("## ") {
            out.push_str(&format!("== {heading}\n"));
        } else if let Some(heading) = trimmed.strip_prefix("# ") {
            out.push_str(&format!("= {heading}\n"));
        } else if let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            out.push_str(&format!("- {}\n", inline_typst(item)));
        } else if trimmed.starts_with("```") {
            out.push_str("```\n");
        } else if trimmed.starts_with('|') {
            out.push_str(&format!("{}\n", trimmed.replace('|', " | ")));
        } else {
            out.push_str(&format!("{}\n\n", inline_typst(trimmed)));
        }
    }
    out
}

fn inline_typst(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('#', "\\#")
        .replace('$', "\\$")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_conversion_handles_headings_and_lists() {
        let typst = markdown_to_typst("# Título\n\n- item\n\nTexto #1");
        assert!(typst.contains("= Título"));
        assert!(typst.contains("- item"));
        assert!(typst.contains("Texto \\#1"));
    }

    #[test]
    fn compiles_csv_to_xlsx() {
        let temp = tempfile::TempDir::new().expect("temp");
        let source = temp.path().join("dados.csv");
        let target = temp.path().join("dados.xlsx");
        std::fs::write(&source, "nome,total\nAna,2\nBia,3\n").expect("csv");
        compile_xlsx(&source, &target).expect("xlsx");
        assert!(target.metadata().expect("meta").len() > 100);
    }

    #[test]
    fn parses_json_rows_with_headers() {
        let rows = parse_json_rows(&serde_json::json!([
            {"nome": "Ana", "total": 2},
            {"nome": "Bia", "total": 3}
        ]));
        assert_eq!(rows.len(), 3);
        assert_eq!(cell_text(&rows[0][0]), "nome");
    }
}
