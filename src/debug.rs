//! Print a plan polygons as SVG.
use std::io::Write;

use crate::poly::Polygons;
use crate::plan::{Lines, PolygonPoint};

pub fn dump_plan(
    mut into: impl Write,
    plan: &Polygons,
    lines: &[Lines],
) -> Result<(), eyre::Report> {
    write!(into, r#"<svg viewBox="-100 -100 200 200" xmlns="http://www.w3.org/2000/svg">"#)?;

    for (window, lines) in plan.windows.iter().zip(lines) {
        write!(into, r#"<polygon points=""#)?;

        for (x, y) in &window.points {
            let x = x * 100.;
            let y = y * 100.;

            write!(into, "{x},{y} ")?;
        }

        write!(into, r#"" fill="none" stroke="black" />"#)?;

        for (origin, r) in lines.ranges.iter().enumerate() {
            for &PolygonPoint(target) in &lines.idx_vec[r.start..r.end] {
                let (x1, y1) = window.points[origin];
                let (x2, y2) = window.points[target];

                let x1 = x1 * 100.;
                let x2 = x2 * 100.;
                let y1 = y1 * 100.;
                let y2 = y2 * 100.;

                write!(into, r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="green" stroke-opacity="40%" />"#)?;
            }
        }
    }

    write!(into, r#"</svg>"#)?;
    Ok(())
}

pub fn dump_output(
    mut into: impl Write,
    plan: &Polygons,
    lines: &[Lines],
) -> Result<(), eyre::Report> {
    write!(into, r#"<svg viewBox="-100 -100 200 200" xmlns="http://www.w3.org/2000/svg">"#)?;

    for (window, lines) in plan.windows.iter().zip(lines) {
        write!(into, r#"<polygon points=""#)?;

        for (x, y) in &window.points {
            let x = x * 100.;
            let y = y * 100.;

            write!(into, "{x},{y} ")?;
        }

        write!(into, r#"" fill="none" stroke="green" />"#)?;

        for w in lines.sequence.windows(2) {
            let &[origin, target] = w.try_into().unwrap();

            let (x1, y1) = window.points[origin.0];
            let (x2, y2) = window.points[target.0];

            let x1 = x1 * 100.;
            let x2 = x2 * 100.;
            let y1 = y1 * 100.;
            let y2 = y2 * 100.;

            write!(into, r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="green" stroke-opacity="40%" />"#)?;
        }
    }

    write!(into, r#"</svg>"#)?;
    Ok(())
}
