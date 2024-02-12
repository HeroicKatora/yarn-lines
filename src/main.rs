mod debug;
mod poly;
mod plan;

use std::path::PathBuf;
use clap::Parser;

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
    let image = image::io::Reader::decode(image)?.into_rgb8();

    let mut sequences = lines.iter().map(|_| plan::RgbSequence::default()).collect::<Vec<_>>();

    if args.rgb {
        for idx in [0, 1, 2] {
            let mut channel = image::GrayImage::new(image.width(), image.height());

            for x in 0..image.width() {
                for y in 0..image.height() {
                    let image::Rgb(ch) = *image.get_pixel(x, y);
                    let black = ch[0].min(ch[1]).min(ch[2]);

                    // We invert the CYMK equivalent chroma.
                    let chroma = (1.0 - eo_transfer(ch[idx])) / (1.0 - eo_transfer(black));
                    channel.put_pixel(x, y, image::Luma([oe_transfer(chroma)]));
                }
            }

            for ((window, lines), rgb) in plan.windows.iter().zip(&mut lines).zip(&mut sequences) {
                // FIXME: the blending mode in planning makes no sense here. We add chroma, but it
                // does luminance planning. If some region is a mix of red/white it won't plan any
                // red but everything else. What.
                let seq = plan::plan(&channel, window, lines)?;
                *rgb.channel(idx) = seq;
            }
        }

        {
            let mut channel = image::GrayImage::new(image.width(), image.height());

            for x in 0..image.width() {
                for y in 0..image.height() {
                    let image::Rgb(ch) = *image.get_pixel(x, y);
                    let black = ch[0].min(ch[1]).min(ch[2]);
                    channel.put_pixel(x, y, image::Luma([black]));
                }
            }

            for ((window, lines), rgb) in plan.windows.iter().zip(&mut lines).zip(&mut sequences) {
                let seq = plan::plan(&channel, window, lines)?;
                rgb.black = seq;
            }
        }

    } else {
        let image = image::DynamicImage::ImageRgb8(image).into_luma8();

        for ((window, lines), rgb) in plan.windows.iter().zip(&mut lines).zip(&mut sequences) {
            let seq = plan::plan(&image, window, lines)?;
            rgb.black = seq;
        }
    }

    debug::dump_output(
        std::fs::File::create(args.debug_plan)?,
        &plan,
        &lines,
        &sequences,
        args.rgb,
    )?;

    Ok(())
}

fn eo_transfer(v: u8) -> f32 {
    (v as f32 / 255.).powf(2.4)
}

fn oe_transfer(v: f32) -> u8 {
    (v.powf(1./2.4) * 255.) as u8
}
