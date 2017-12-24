#[macro_use]
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate mdbook;
extern crate failure;

use failure::{ResultExt, Error, SyncFailure};
use mdbook::renderer::RenderContext;


/// Test each of the Rust snippets in a book.
/// 
/// This works by:
/// 
/// - Creating a new crate in a temporary directory
/// - Copying across the book chapters as `*.md` files
/// - Update `Cargo.toml` to include any extra dependencies the user specifies
/// - Run [rust-skeptic] over the generated crate
/// 
/// [rust-skeptic]: https://github.com/budziq/rust-skeptic
pub fn test(ctx: &RenderContext) -> Result<(), Error> {
    let cfg: Config = ctx.config.get_deserialized("output.test")
        .map_err(SyncFailure::new)?;

    unimplemented!()
}


/// The configuration struct loaded from the `output.test` table.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
    pub dependencies: Vec<String>,
}