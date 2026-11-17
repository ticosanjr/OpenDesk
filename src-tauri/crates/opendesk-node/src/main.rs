//! Remote worker wrapper. The worker implementation lives in the shell CLI;
//! this binary forces the `node run` command and stays Tauri-free.
fn main() {
    let mut args = vec!["node".to_string(), "run".to_string()];
    args.extend(std::env::args().skip(1));
    opendesk_lib::run_cli(args);
}
