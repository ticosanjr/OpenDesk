mod archive;
mod list;
mod media;
mod paths;
mod raw;
mod read;
mod save;
mod search;

pub use archive::{ArchiveEntry, archive, archive_entries};
#[allow(unused_imports)]
pub use list::{FileEntry, tree};
pub use media::media;
#[allow(unused_imports)]
pub use paths::FileQuery;
#[allow(unused_imports)]
pub use raw::{RawChunk, raw, read_raw};
#[allow(unused_imports)]
pub use read::{FileContent, content};
#[allow(unused_imports)]
pub use save::{SaveFileBody, save_file, save_to_destination};
#[allow(unused_imports)]
pub use search::{FileSearchResult, invalidate_search_index, search};

#[cfg(test)]
#[path = "../files_tests.rs"]
mod tests;
