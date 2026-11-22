use crate::parser::{FlowItem, Step};

pub(super) fn previous_step(items: &[FlowItem], index: usize) -> Option<(usize, &Step)> {
    items[..index]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(position, item)| match item {
            FlowItem::Step(step) => Some((position, step)),
            _ => None,
        })
}

pub(super) fn next_item_name(items: &[FlowItem], index: usize) -> Option<&str> {
    items.get(index).and_then(|item| match item {
        FlowItem::Step(step) => Some(step.name.as_str()),
        FlowItem::Hitl(checkpoint) => Some(checkpoint.id.as_str()),
        FlowItem::Sink(sink) => sink.name.as_deref(),
        FlowItem::Branch(branch) => Some(branch.name.as_deref().unwrap_or(&branch.id)),
        FlowItem::Parallel(parallel) => Some(parallel.name.as_deref().unwrap_or(&parallel.id)),
        FlowItem::Map(map) => Some(map.name.as_deref().unwrap_or(&map.id)),
        FlowItem::Recipe(call) => Some(call.name.as_deref().unwrap_or(&call.id)),
        FlowItem::Loop(loop_flow) => Some(loop_flow.name.as_deref().unwrap_or(&loop_flow.id)),
        FlowItem::Switch(switch) => Some(switch.name.as_deref().unwrap_or(&switch.id)),
    })
}
