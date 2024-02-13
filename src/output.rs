use std::io::Write;
use std::path::PathBuf;

use crate::{
    plan::Lines,
    plan::RgbSequence,
    poly::Polygons,
};

pub struct Files {
    pub section_mask_svg: PathBuf,
    pub section_list: PathBuf,
}

impl Files {
    pub(crate) fn dump(
        &self,
        (w, h): (u32, u32),
        plan: &Polygons,
        lines: &[Lines],
        sequences: &[RgbSequence],
    ) -> Result<(), eyre::Report> {
        Self::dump_mask(
            std::fs::File::create(&self.section_mask_svg)?,
            (w, h),
            plan,
            lines,
        )?;

        Ok(())
    }

    fn dump_mask(
        mut into: impl Write,
        (w, h): (u32, u32),
        plan: &Polygons,
        lines: &[Lines],
    ) -> Result<(), eyre::Report> {
        let (w, h) = (w as f32 * 2.0, h as f32 * 2.0);
        let (lx, ly) = (-w / 2.0, -h / 2.0);
        write!(into, r#"<svg viewBox="{lx} {ly} {w} {h}" xmlns="http://www.w3.org/2000/svg">"#)?;
        let (w, h) = (w / 2.0, h / 2.0);

        for (idx, (window, _lines)) in plan.windows.iter().zip(lines).enumerate() {
            write!(into, r#"<polygon points=""#)?;

            let (mut sum_x, mut sum_y) = (0.0, 0.0);
            for (x, y) in &window.points {
                let x = x * w;
                let y = y * h;

                sum_x += x;
                sum_y += y;

                write!(into, "{x},{y} ")?;
            }

            write!(into, r#"" fill="none" stroke="black" />"#)?;

            if !window.points.is_empty() {
                let avg_x = sum_x / window.points.len() as f32;
                let avg_y = sum_y / window.points.len() as f32;
                let label = idx;

                write!(into, r#"<text text-anchor="middle" x="{avg_x}" y="{avg_y}">{label}</text>"#)?;
            }
        }

        write!(into, r#"</svg>"#)?;
        Ok(())
    }

}
