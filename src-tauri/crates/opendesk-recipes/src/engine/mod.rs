//! Sequential execution driver for flat recipes (`recipe.flow`).

mod execute;
mod finalize;
mod lifecycle;
mod steps;

pub use lifecycle::{resume, start};

pub(crate) use finalize::{finish, mark_routine, replace_attachment};
pub(crate) use steps::execute_step;

#[cfg(test)]
mod tests;
