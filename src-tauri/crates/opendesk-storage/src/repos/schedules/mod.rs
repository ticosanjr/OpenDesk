//! Routine storage: schedule types, reads and mutations.

mod model;
mod mutations;

pub use model::*;
pub use mutations::*;

#[cfg(test)]
mod tests;
