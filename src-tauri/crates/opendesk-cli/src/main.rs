//! Headless CLI. It reuses the shell argument parser and never initializes the
//! desktop UI.

mod export;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) == Some("migrate-v1-export") {
        if let Err(error) = export::run(&args[1..]) {
            eprintln!("migrate-v1-export: {error}");
            std::process::exit(1);
        }
        return;
    }
    opendesk_lib::run_cli(args);
}
