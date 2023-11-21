use clap::Parser;
use sqlweld::{build, Error, Options};

fn main() -> Result<(), error_stack::Report<Error>> {
    let options = Options::parse();
    build(options)
}
