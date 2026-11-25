//! `opendesk repair-db`: probe, back up and safely repair the local SQLite.

use opendesk_storage::paths::AppPaths;
use std::{path::PathBuf, process::exit};

const HELP: &str = "uso: opendesk repair-db [--db <arquivo>]";

pub fn run(args: Vec<String>) {
    if let Err(error) = execute(&args) {
        eprintln!("repair-db: {error}\n{HELP}");
        exit(1);
    }
}

fn execute(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{HELP}");
        return Ok(());
    }
    let mut db: Option<PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--db" => {
                index += 1;
                db = Some(PathBuf::from(
                    args.get(index).ok_or("--db exige um caminho")?,
                ));
            }
            other => return Err(format!("argumento desconhecido '{other}'")),
        }
        index += 1;
    }
    let paths = AppPaths::discover().map_err(|error| format!("{error:#}"))?;
    let db = db.unwrap_or_else(|| paths.db_path.clone());
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    let report = runtime
        .block_on(opendesk_storage::integrity::ensure_database_integrity(&db))
        .map_err(|error| format!("{error}"))?;
    if report.ok {
        println!(
            "banco íntegro: {} (journal {})",
            db.display(),
            report.journal_mode
        );
        Ok(())
    } else {
        Err(format!(
            "banco continua inválido: {}",
            report.errors.join("; ")
        ))
    }
}
