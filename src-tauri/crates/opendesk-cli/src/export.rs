//! `opendesk-cli migrate-v1-export`: archival bridge out of a V1 database.
//!
//! The implementation lives in `opendesk-storage::legacy` so the desktop
//! binary can offer the same export before starting with a fresh V2 database.

use opendesk_storage::legacy::export_v1;
use std::path::PathBuf;

const HELP: &str =
    "uso: opendesk-cli migrate-v1-export --db <v1.db> --out <dir> [--format json|yaml]";

pub fn run(args: &[String]) -> Result<(), String> {
    if args.is_empty() || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Err(HELP.into());
    }
    let (db, out, yaml) = parse(args)?;
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    let summary = runtime
        .block_on(export_v1(&db, &out, yaml))
        .map_err(|error| format!("{error:#}"))?;
    println!(
        "exportado: {} workspace(s), {} receita(s), {} setting(s) em {}",
        summary.workspaces,
        summary.recipes,
        summary.settings,
        summary.out.display()
    );
    Ok(())
}

fn parse(args: &[String]) -> Result<(PathBuf, PathBuf, bool), String> {
    let mut db = None;
    let mut out = None;
    let mut yaml = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--db" => {
                index += 1;
                db = Some(PathBuf::from(
                    args.get(index).ok_or("--db exige um caminho")?,
                ));
            }
            "--out" => {
                index += 1;
                out = Some(PathBuf::from(
                    args.get(index).ok_or("--out exige um diretório")?,
                ));
            }
            "--format" => {
                index += 1;
                yaml = match args.get(index).map(String::as_str) {
                    Some("json") => false,
                    Some("yaml") => true,
                    Some(other) => return Err(format!("formato inválido '{other}' (json|yaml)")),
                    None => return Err("--format exige json ou yaml".into()),
                };
            }
            other => return Err(format!("argumento desconhecido '{other}'\n{HELP}")),
        }
        index += 1;
    }
    Ok((
        db.ok_or("--db é obrigatório")?,
        out.ok_or("--out é obrigatório")?,
        yaml,
    ))
}
