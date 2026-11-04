//! Tags kept with the files themselves, in the `user.xdg.tags` attribute.

pub mod codec;
pub mod error;
pub mod query;
pub mod xattr;

pub use codec::{Tags, decode_tags, encode_tags};
pub use error::TagError;
pub use query::SearchQuery;
pub use xattr::{TagReader, add_tag, read_tags, read_tags_in, remove_tag, toggle_tag, write_tags};
