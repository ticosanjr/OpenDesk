//! Structured workspace memory: namespaces, ACL and notes.

mod model;
mod notes;

pub use model::*;
pub use notes::*;

#[cfg(test)]
mod tests;
