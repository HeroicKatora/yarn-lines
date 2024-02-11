mod debug;
mod poly;
mod plan;

use std::path::PathBuf;
use clap::Parser;

#[derive(Parser)]
struct Args {
    image: PathBuf,
    #[clap(default_value = "examples/c2.json")]
    circle: PathBuf,
    #[clap(default_value = "target/template.svg")]
    debug_template: PathBuf,
    #[clap(default_value = "target/plan.svg")]
    debug_plan: PathBuf,
}

fn main() -> Result<(), eyre::Report> {
    let args = Args::parse();

    let plan = poly::read({
        std::fs::File::open(args.circle)?
    })?;

    let mut lines = vec![];
    for window in &plan.windows {
        lines.push(plan::permissible_lines(&window));
    }

    let image = image::io::Reader::new({
        let file = std::fs::File::open(args.image)?;
        std::io::BufReader::new(file)
    });

    debug::dump_plan(
        std::fs::File::create(args.debug_template)?,
        &plan,
        &lines,
    )?;

    let image = image::io::Reader::with_guessed_format(image)?;
    let image = image::io::Reader::decode(image)?.into_luma8();

    for (window, lines) in plan.windows.iter().zip(&mut lines) {
        plan::plan(&image, window, lines)?;
    }

    debug::dump_output(
        std::fs::File::create(args.debug_plan)?,
        &plan,
        &lines,
    )?;

    Ok(())
}
