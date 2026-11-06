pub mod classifier;
pub mod context_packer;
pub mod meta_builder;

pub use classifier::{RouterDecision, RouterInput, classify, output_directory, slugify};
pub use context_packer::{PackContext, pack};
pub use meta_builder::{
    MetaBuildRequest, MetaBuildResult, build as meta_build, detect as meta_detect,
};
