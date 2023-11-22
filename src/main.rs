use std::panic::Location;

use clap::Parser;
use sqlweld::{build, Error, Options};

fn main() -> Result<(), error_stack::Report<Error>> {
    #[cfg(debug_assertions)]
    let hide_file_locs = false;
    #[cfg(not(debug_assertions))]
    let hide_file_locs = std::env::var("RUST_BACKTRACE").is_err();

    if hide_file_locs {
        error_stack::Report::install_debug_hook::<Location>(|_, _| {});
    }

    let options = Options::parse();
    build(options)
}
