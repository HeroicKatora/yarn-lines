mod atomicf32;
mod color;
mod debug;
mod output;
mod poly;
mod plan;

use core::sync::atomic::{AtomicU32, Ordering};
use atomicf32::AtomicF32;
use std::path::PathBuf;
use clap::Parser;

use rayon::prelude::{ParallelBridge, IntoParallelIterator, ParallelIterator};

#[derive(Parser)]
struct Args {
    #[clap(long = "rgb", default_value = "false")]
    rgb: bool,
    image: PathBuf,
    #[clap(long = "circle", default_value = "examples/c2.json")]
    circle: PathBuf,
    #[clap(long = "debug-template", default_value = "target/template.svg")]
    debug_template: PathBuf,
    #[clap(long = "debug-plan", default_value = "target/plan.svg")]
    debug_plan: PathBuf,
}

fn main() -> Result<(), eyre::Report> {
    let args = Args::parse();

    let plan = poly::read({
        std::fs::File::open(args.circle)?
    })?;

    let image = image::io::Reader::new({
        let file = std::fs::File::open(args.image)?;
        std::io::BufReader::new(file)
    });

    let image = image::io::Reader::with_guessed_format(image)?;
    let image = image::io::Reader::decode(image)?.into_rgb8();
    let dimensions = image.dimensions();

    let mut lines = vec![];
    for window in &plan.windows {
        lines.push(plan::permissible_lines(&window, dimensions));
    }

    debug::dump_plan(
        std::fs::File::create(args.debug_template)?,
        dimensions,
        &plan,
        &lines,
    )?;

    let mut sequences = lines.iter().map(|_| plan::RgbSequence::default()).collect::<Vec<_>>();

    let preliminary_break = AtomicU32::new(0);
    let regions_covered = AtomicU32::new(0);
    let yarn_length = AtomicF32::new();

    let primary: color::PrimaryBase = plan.primaries.to_color_base();

    if args.rgb {
        let color_plan = color::decouple(&image, &primary);
        let channels = [&color_plan.red, &color_plan.green, &color_plan.blue];

        for (idx, channel) in [0, 1, 2].into_iter().zip(channels) {
            let tasks = plan.windows.iter().zip(&mut lines).zip(&mut sequences);

            tasks
                .par_bridge()
                .into_par_iter()
                .try_for_each(|((window, lines), rgb)| {
                    // FIXME: the blending mode in planning makes no sense here. We add chroma, but it
                    // does luminance planning. If some region is a mix of red/white it won't plan any
                    // red but everything else. What.
                    let seq = plan::plan(&channel, window, lines)?;

                    preliminary_break
                        .fetch_add(
                            u32::from(matches!(seq.break_reason, plan::BreakReason::EndOfIteration)),
                            Ordering::Relaxed,
                        );
                    regions_covered.fetch_add(1, Ordering::Relaxed);
                    yarn_length.fetch_add(seq.yarn_length);

                    *rgb.channel(idx) = seq;
                    Ok::<_, eyre::Report>(())
                })?;
        }

        {
            let channel = &color_plan.gray;
            let tasks = plan.windows.iter().zip(&mut lines).zip(&mut sequences);

            tasks
                .par_bridge()
                .into_par_iter()
                .try_for_each(|((window, lines), rgb)| {
                    let seq = plan::plan(&channel, window, lines)?;

                    preliminary_break
                        .fetch_add(
                            u32::from(matches!(seq.break_reason, plan::BreakReason::EndOfIteration)),
                            Ordering::Relaxed,
                        );
                    regions_covered.fetch_add(1, Ordering::Relaxed);
                    yarn_length.fetch_add(seq.yarn_length);

                    rgb.black = seq;
                    Ok::<_, eyre::Report>(())
                })?;
        }

    } else {
        let image = image::DynamicImage::ImageRgb8(image).into_luma8();

        let tasks = plan.windows.iter().zip(&mut lines).zip(&mut sequences);

        tasks
            .par_bridge()
            .into_par_iter()
            .try_for_each(|((window, lines), rgb)| {
                let seq = plan::plan(&image, window, lines)?;

                preliminary_break
                    .fetch_add(
                        u32::from(matches!(seq.break_reason, plan::BreakReason::EndOfIteration)),
                        Ordering::Relaxed,
                    );
                regions_covered.fetch_add(1, Ordering::Relaxed);
                yarn_length.fetch_add(seq.yarn_length);

                rgb.black = seq;
                    Ok::<_, eyre::Report>(())
            })?;
    }

    let preliminary_break = preliminary_break.load(Ordering::Relaxed);
    let regions_covered = regions_covered.load(Ordering::Relaxed);

    if preliminary_break > 0 {
        eprintln!("Regions not covered: {preliminary_break} / {regions_covered}");
    }

    let yarn_length = yarn_length.load();

    // Scale to height = 0.5m
    let metric_yarn = yarn_length / (dimensions.1 as f32) * 50. / 100.;
    eprintln!("Yarn: {metric_yarn:.3} m");

    debug::dump_output(
        std::fs::File::create(args.debug_plan)?,
        dimensions,
        &plan,
        &lines,
        &sequences,
        &primary,
        args.rgb,
    )?;

    let output = output::Files {
        section_mask_svg: "target/mask.svg".into(),
        section_list: "target/sections.json".into(),
    };

    output.dump(
        dimensions,
        &plan,
        &lines,
        &sequences,
    )?;

    Ok(())
}

fn eo_transfer(v: u8) -> f32 {
    (v as f32 / 255.).powf(2.4)
}

fn oe_transfer(v: f32) -> u8 {
    (v.powf(1./2.4) * 255.) as u8
}
