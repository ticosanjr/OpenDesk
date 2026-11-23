use super::DoctorReport;

pub(super) fn print_human(report: &DoctorReport) {
    println!("OpenDesk {} — doctor\n", report.version);
    println!("data:    {}", report.data_dir);
    println!("config:  {}", report.config_dir);
    println!("logs:    {}", report.log_dir);
    println!(
        "MCP ctl: {}/{} workspaces · pacote base: {}/{}",
        report.mcp_control_workspaces,
        report.workspaces,
        report.base_pack_workspaces,
        report.workspaces
    );
    println!(
        "database: {} ({} workspaces)",
        if report.database_ok {
            "ok"
        } else {
            "unavailable"
        },
        report.workspaces
    );

    match &report.binary {
        Some(binary) => println!(
            "opencode: {} (v{} · método {})",
            binary.path,
            binary.version,
            binary.method.as_deref().unwrap_or("desconhecido")
        ),
        None => println!(
            "opencode: não encontrado — {}",
            report
                .binary_error
                .as_deref()
                .unwrap_or("nenhum caminho testado funcionou")
        ),
    }

    match &report.daemon {
        Some(probe) => println!(
            "daemon: {} — {}",
            probe.source,
            if probe.online {
                format!("online v{}", probe.version.as_deref().unwrap_or("?"))
            } else {
                format!(
                    "offline ({})",
                    probe.error.as_deref().unwrap_or("sem detalhe")
                )
            }
        ),
        None => println!("daemon: sem daemon.json (nenhum daemon dedicado iniciado)"),
    }

    for probe in &report.shared_services {
        println!(
            "serviço: {} — {}",
            probe.source,
            if probe.online {
                format!("online v{}", probe.version.as_deref().unwrap_or("?"))
            } else {
                format!(
                    "offline ({})",
                    probe.error.as_deref().unwrap_or("sem detalhe")
                )
            }
        );
    }

    if report.warnings.is_empty() {
        println!("\nTudo certo.");
    } else {
        println!("\nAvisos:");
        for warning in &report.warnings {
            println!("  - {warning}");
        }
    }
}
