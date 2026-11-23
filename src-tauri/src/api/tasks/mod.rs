use crate::{
    app_state::AppState,
    db,
    error::{AppError, AppResult},
    opencode_client::rest_client::ModelRefDto,
};

pub(crate) mod task_dispatch;

mod crud;
mod diagnostics;
mod dispatch;
mod fork;
mod memory;
mod messages;
mod transition;

pub(crate) use task_dispatch::{dispatch_task_internal, dispatch_task_with_session};

#[allow(unused_imports)]
pub use crud::{
    MoveBody, TaskQuery, create_task, delete_task, get_task, list_tasks, move_task, session_packet,
    update_task,
};
pub use diagnostics::task_diagnostics;
#[allow(unused_imports)]
pub use dispatch::{
    DispatchBody, SwarmBody, approve_plan, dispatch_task, retry_other_model, swarm_task,
};
#[allow(unused_imports)]
pub use fork::{ForkBody, fork_from_run, fork_task};
#[allow(unused_imports)]
pub use memory::{MemoryBody, MemoryQuery, delete_task_memory, get_task_memory, post_task_memory};
pub use messages::{list_task_messages, post_task_message};
#[allow(unused_imports)]
pub use transition::{HoldBody, TransitionBody, hold_task, transition_task};

pub async fn resolve_workspace_id(state: &AppState, requested: Option<&str>) -> AppResult<String> {
    if let Some(id) = requested {
        return Ok(id.to_string());
    }
    db::settings::active_workspace_id(&state.db)
        .await?
        .ok_or_else(|| AppError::BadRequest("no active workspace".into()))
}

pub(crate) fn validate_model_ref(value: Option<&str>) -> AppResult<()> {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty())
        && ModelRefDto::parse(value).is_none()
    {
        return Err(AppError::BadRequest(format!(
            "modelo inválido '{value}'; use provider/modelo ou provider/modelo#variant"
        )));
    }
    Ok(())
}
