use crate::{
    error::{AppError, AppResult},
    io_bus::tools,
};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct ExtractedDocument {
    pub text: String,
}

pub fn extract_document(path: &Path) -> AppResult<ExtractedDocument> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "csv" => Ok(ExtractedDocument {
            text: std::fs::read_to_string(path)?,
        }),
        "json" | "md" | "txt" | "typ" => Ok(ExtractedDocument {
            text: std::fs::read_to_string(path)?,
        }),
        "xlsx" | "xls" => {
            use calamine::{Reader, open_workbook_auto};
            let mut workbook = open_workbook_auto(path)
                .map_err(|error| AppError::BadRequest(format!("planilha inválida: {error}")))?;
            let mut text = String::new();
            for sheet in workbook.sheet_names().to_owned() {
                text.push_str(&format!("# {sheet}\n\n"));
                if let Ok(range) = workbook.worksheet_range(&sheet) {
                    for row in range.rows() {
                        text.push_str(
                            &row.iter()
                                .map(|cell| cell.to_string())
                                .collect::<Vec<_>>()
                                .join(" | "),
                        );
                        text.push('\n');
                    }
                }
                text.push('\n');
            }
            Ok(ExtractedDocument { text })
        }
        "pdf" => {
            let text = pdf_extract::extract_text(path)
                .map_err(|error| AppError::BadRequest(format!("PDF inválido: {error}")))?;
            Ok(ExtractedDocument { text })
        }
        "docx" => extract_docx(path),
        other => Err(AppError::BadRequest(format!(
            "extensão de documento não suportada: {other}"
        ))),
    }
}

fn extract_docx(path: &Path) -> AppResult<ExtractedDocument> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| AppError::BadRequest(format!("DOCX inválido: {error}")))?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|error| AppError::BadRequest(format!("DOCX sem document.xml: {error}")))?
        .read_to_string(&mut xml)?;
    let mut text = String::new();
    let mut in_tag = false;
    let mut tag = String::new();
    for character in xml.chars() {
        match character {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' => {
                in_tag = false;
                if tag.starts_with("/w:p") {
                    text.push('\n');
                }
                if tag.starts_with("w:tab") {
                    text.push('\t');
                }
            }
            _ if in_tag => tag.push(character),
            _ => text.push(character),
        }
    }
    Ok(ExtractedDocument { text })
}

pub async fn ocr(path: &Path, languages: &str) -> AppResult<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(120),
        tokio::process::Command::new(tools::command("tesseract"))
            .arg(path)
            .args(["stdout", "-l", languages])
            .output(),
    )
    .await
    .map_err(|_| AppError::Internal("OCR timeout".into()))?
    .map_err(|error| AppError::BadRequest(format!("tesseract indisponível: {error}")))?;
    if !output.status.success() {
        return Err(AppError::Internal(format!(
            "tesseract falhou: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub async fn transcribe(path: &Path, model: Option<&Path>) -> AppResult<String> {
    let mut command = tokio::process::Command::new(tools::command("whisper-cli"));
    command.arg("-f").arg(path).args(["-otxt", "-of"]);
    let output_base = path.with_extension("transcript");
    command.arg(&output_base);
    if let Some(model) = model {
        command.arg("-m").arg(model);
    }
    let output = tokio::time::timeout(Duration::from_secs(30 * 60), command.output())
        .await
        .map_err(|_| AppError::Internal("transcrição timeout".into()))?
        .map_err(|error| AppError::BadRequest(format!("whisper-cli indisponível: {error}")))?;
    if !output.status.success() {
        return Err(AppError::Internal(format!(
            "whisper-cli falhou: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let transcript = output_base.with_extension("txt");
    let text = std::fs::read_to_string(&transcript).unwrap_or_default();
    let _ = std::fs::remove_file(output_base);
    let _ = std::fs::remove_file(transcript);
    Ok(text)
}

pub fn move_processed(path: &Path, workspace_root: &Path) -> AppResult<PathBuf> {
    let folder = workspace_root
        .join("input")
        .join("processed")
        .join(chrono::Utc::now().format("%Y-%m").to_string());
    std::fs::create_dir_all(&folder)?;
    let name = path.file_name().unwrap_or_default();
    let destination = folder.join(name);
    std::fs::rename(path, &destination)?;
    Ok(destination)
}

pub fn move_failed(path: &Path, workspace_root: &Path) -> AppResult<PathBuf> {
    let folder = workspace_root.join("input").join("failed");
    std::fs::create_dir_all(&folder)?;
    let name = path.file_name().unwrap_or_default();
    let destination = folder.join(name);
    std::fs::rename(path, &destination)?;
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extracts_markdown_csv_and_json() {
        let temp = TempDir::new().expect("temp");
        let md = temp.path().join("a.md");
        std::fs::write(&md, "# Título").expect("md");
        assert_eq!(extract_document(&md).expect("md").text, "# Título");
        let csv = temp.path().join("a.csv");
        std::fs::write(&csv, "a,b\n1,2").expect("csv");
        assert!(extract_document(&csv).expect("csv").text.contains("1,2"));
        let json = temp.path().join("a.json");
        std::fs::write(&json, "{\"a\":1}").expect("json");
        assert!(
            extract_document(&json)
                .expect("json")
                .text
                .contains("\"a\"")
        );
    }

    #[test]
    fn extracts_docx_text() {
        use std::io::Write;
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join("a.docx");
        let file = std::fs::File::create(&path).expect("docx");
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .expect("entry");
        zip.write_all(b"<w:document><w:p>Ola</w:p></w:document>")
            .expect("body");
        zip.finish().expect("finish");
        assert!(
            extract_document(&path)
                .expect("extract")
                .text
                .contains("Ola")
        );
    }

    #[test]
    fn moves_processed_and_failed() {
        let temp = TempDir::new().expect("temp");
        let root = temp.path();
        std::fs::create_dir_all(root.join("input")).expect("input");
        let processed = root.join("input/a.txt");
        std::fs::write(&processed, "a").expect("a");
        assert!(
            move_processed(&processed, root)
                .expect("processed")
                .exists()
        );
        assert!(!processed.exists());
        let failed = root.join("input/b.txt");
        std::fs::write(&failed, "b").expect("b");
        assert!(move_failed(&failed, root).expect("failed").exists());
    }
}
