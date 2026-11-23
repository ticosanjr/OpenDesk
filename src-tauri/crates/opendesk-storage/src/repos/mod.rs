//! Repositories. `conversations` is the V2 unified stream; the remaining V1
//! repositories move here as their engine/recipe owners are extracted.

pub mod ab;
pub mod artifacts;
pub mod auth_sessions;
pub mod budget;
pub mod calendar;
pub mod channels;
pub mod chat;
pub mod chat_turns;
pub mod conversations;
pub mod deliveries;
pub mod handoffs;
pub mod integrations;
pub mod interactions;
pub mod io_bus;
pub mod memory;
pub mod missed_runs;
pub mod model_health;
pub mod models;
pub mod permissions;
pub mod proposals;
pub mod push;
pub mod queue;
pub mod recipe_files;
pub mod router;
pub mod runs;
pub mod schedules;
pub mod settings;
pub mod task_dedupe;
pub mod task_messages;
pub mod task_watchers;
pub mod tasks;
pub mod tickets;
pub mod vault;
pub mod workspaces;
