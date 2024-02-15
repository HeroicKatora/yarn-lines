use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use serde::Serialize;

use crate::{
    plan::Lines,
    plan::{RgbSequence, PolygonPoint},
    poly::Polygons,
};

pub struct Files {
    pub section_mask_svg: PathBuf,
    pub section_list: PathBuf,
}

#[derive(Default, Serialize)]
pub struct Plan {
    pub red_length_in_m: f32,
    pub nodes_red: Vec<String>,
    pub green_length_in_m: f32,
    pub nodes_green: Vec<String>,
    pub blue_length_in_m: f32,
    pub nodes_blue: Vec<String>,
    pub gray_length_in_m: f32,
    pub nodes_gray: Vec<String>,
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

        let yarn_factor = 1.0 / (w as f32) * 50. / 100.;

        Self::dump_plan(
            std::fs::File::create(&self.section_list)?,
            plan,
            lines,
            sequences,
            yarn_factor,
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

    fn dump_plan(
        into: impl Write,
        plan: &Polygons,
        lines: &[Lines],
        sequences: &[RgbSequence],
        yarn_factor: f32,
    ) -> Result<(), eyre::Report> {
        let _ = (plan, lines, sequences);

        let labeled = plan.windows.iter().zip(lines).zip(sequences);
        let mut hash = HashMap::new();

        for (idx, ((window, _lines), seq)) in labeled.enumerate() {
            let label = idx;

            let name_of = |idx: PolygonPoint| -> String {
                window.names[idx.0].clone()
            };


            let mut plan = Plan::default();
            for win in seq.r.sequence.windows(2) {
                let &[_, index] = win.try_into().unwrap();
                plan.nodes_red.push(name_of(index));
            }

            for win in seq.g.sequence.windows(2) {
                let &[_, index] = win.try_into().unwrap();
                plan.nodes_green.push(name_of(index));
            }

            for win in seq.b.sequence.windows(2) {
                let &[_, index] = win.try_into().unwrap();
                plan.nodes_blue.push(name_of(index));
            }

            for win in seq.black.sequence.windows(2) {
                let &[_, index] = win.try_into().unwrap();
                plan.nodes_gray.push(name_of(index));
            }

            plan.red_length_in_m = seq.r.yarn_length * yarn_factor;
            plan.green_length_in_m = seq.g.yarn_length * yarn_factor;
            plan.blue_length_in_m = seq.b.yarn_length * yarn_factor;
            plan.gray_length_in_m = seq.black.yarn_length * yarn_factor;
            hash.insert(label.to_string(), plan);
        }

        serde_json::to_writer(into, &hash)?;
        Ok(())
    }
}
