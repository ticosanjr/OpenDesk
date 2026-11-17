//! Headless CLI. It reuses the shell argument parser and never initializes the
//! desktop UI.
fn main() {
    opendesk_lib::run_cli(std::env::args().skip(1).collect());
}
