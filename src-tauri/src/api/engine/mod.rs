mod lifecycle;
mod logs;
mod models;
mod update;

pub use lifecycle::{detect, require_engine, require_engine_strict, restart, set_binary, status};
pub use logs::diagnostics;
pub use models::{clear_model_health, model_health, test_model};
pub use update::{rollback, update, update_check};

#[cfg(test)]
#[path = "../engine_tests.rs"]
mod tests;
