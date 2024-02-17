//! Print a plan polygons as SVG.
use std::io::Write;

use crate::color::PrimaryBase;
use crate::poly::Polygons;
use crate::plan::{Lines, PolygonPoint, RgbSequence};

pub fn dump_plan(
    mut into: impl Write,
    (w, h): (u32, u32),
    plan: &Polygons,
    lines: &[Lines],
) -> Result<(), eyre::Report> {
    let (w, h) = (w as f32, h as f32);
    let (lx, ly) = (-w / 2.0, -h / 2.0);
    write!(into, r#"<svg viewBox="{lx} {ly} {w} {h}" xmlns="http://www.w3.org/2000/svg">"#)?;
    let (w, h) = (w / 2.0, h / 2.0);

    for (window, lines) in plan.windows.iter().zip(lines) {
        write!(into, r#"<polygon points=""#)?;

        for (x, y) in &window.points {
            let x = x * w;
            let y = y * h;

            write!(into, "{x},{y} ")?;
        }

        write!(into, r#"" fill="none" stroke="black" />"#)?;

        for (origin, r) in lines.ranges.iter().enumerate() {
            for &PolygonPoint(target) in &lines.idx_vec[r.start..r.end] {
                let (x1, y1) = window.points[origin];
                let (x2, y2) = window.points[target];

                let x1 = x1 * w;
                let x2 = x2 * w;
                let y1 = y1 * h;
                let y2 = y2 * h;

                write!(into, r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="green" stroke-opacity="40%" />"#)?;
            }
        }
    }

    write!(into, r#"</svg>"#)?;
    Ok(())
}

pub fn dump_output(
    mut into: impl Write,
    (w, h): (u32, u32),
    plan: &Polygons,
    lines: &[Lines],
    sequences: &[RgbSequence],
    primary: &PrimaryBase,
    is_rgbish: bool,
) -> Result<(), eyre::Report> {
    let (w, h) = (w as f32, h as f32);
    let (lx, ly) = (-w / 2.0, -h / 2.0);
    write!(into, r#"<svg viewBox="{lx} {ly} {w} {h}" xmlns="http://www.w3.org/2000/svg">"#)?;
    let (w, h) = (w / 2.0, h / 2.0);

    for ((window, _lines), rgb) in plan.windows.iter().zip(lines).zip(sequences) {
        write!(into, r#"<polygon points=""#)?;

        for (x, y) in &window.points {
            let x = x * w;
            let y = y * h;

            write!(into, "{x},{y} ")?;
        }

        write!(into, r#"" fill="none" stroke="green" />"#)?;

        for bw in rgb.black.sequence.windows(2) {
            let &[origin, target] = bw.try_into().unwrap();

            let (x1, y1) = window.points[origin.0];
            let (x2, y2) = window.points[target.0];

            let x1 = x1 * w;
            let x2 = x2 * w;
            let y1 = y1 * h;
            let y2 = y2 * h;

            write!(into, r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="black" stroke-opacity="40%" />"#)?;
        }

        if !is_rgbish {
            continue;
        }

        let c_blue = format_color_css(&primary.blue);
        for bb in rgb.b.sequence.windows(2) {
            let &[origin, target] = bb.try_into().unwrap();

            let (x1, y1) = window.points[origin.0];
            let (x2, y2) = window.points[target.0];

            let x1 = x1 * w;
            let x2 = x2 * w;
            let y1 = y1 * h;
            let y2 = y2 * h;

            write!(into, r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{c_blue}" stroke-opacity="80%" />"#)?;
        }

        let c_green = format_color_css(&primary.green);
        for bg in rgb.g.sequence.windows(2) {
            let &[origin, target] = bg.try_into().unwrap();

            let (x1, y1) = window.points[origin.0];
            let (x2, y2) = window.points[target.0];

            let x1 = x1 * w;
            let x2 = x2 * w;
            let y1 = y1 * h;
            let y2 = y2 * h;

            write!(into, r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{c_green}" stroke-opacity="80%" />"#)?;
        }

        let c_red = format_color_css(&primary.red);
        for br in rgb.r.sequence.windows(2) {
            let &[origin, target] = br.try_into().unwrap();

            let (x1, y1) = window.points[origin.0];
            let (x2, y2) = window.points[target.0];

            let x1 = x1 * w;
            let x2 = x2 * w;
            let y1 = y1 * h;
            let y2 = y2 * h;

            write!(into, r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{c_red}" stroke-opacity="80%" />"#)?;
        }

        for (idx, bw) in rgb.black.sequence.windows(2).enumerate() {
            if idx % 4 != 0 {
                continue;
            }

            let &[origin, target] = bw.try_into().unwrap();

            let (x1, y1) = window.points[origin.0];
            let (x2, y2) = window.points[target.0];

            let x1 = x1 * w;
            let x2 = x2 * w;
            let y1 = y1 * h;
            let y2 = y2 * h;

            write!(into, r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="black" stroke-opacity="40%" />"#)?;
        }

    }

    write!(into, r#"</svg>"#)?;
    Ok(())
}

fn format_color_css(color: &image::Rgb<u8>) -> String {
    let [r, g, b] = color.0;
    format!("#{r:02x}{g:02x}{b:02x}")
}
