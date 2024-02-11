mod poly;
mod plan;

use std::path::PathBuf;
use clap::Parser;

#[derive(Parser)]
struct Args {
    image: PathBuf,
    circle: PathBuf,
}

fn main() -> Result<(), eyre::Report> {
    let args = Args::parse();

    let polys = poly::read({
        std::fs::File::open(args.circle)?
    })?;

    Ok(())
}
