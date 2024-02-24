use std::ops::Range;

use image::{GenericImage, GenericImageView, GrayImage};
use imageproc::{rect::Rect, point::Point};
use rand_xoshiro::{
    rand_core::SeedableRng,
    rand_core::RngCore,
    Xoshiro128Plus,
};

use crate::poly::Polygon;
use crate::{eo_transfer, oe_transfer};

pub struct LineClass {
    pub of: usize,
    pub idx: usize,
}

#[derive(Default)]
pub struct Lines {
    pub idx_vec: Vec<PolygonPoint>,
    /// Weights of the lines in `idx_vec` (a coverage metric).
    pub weight_vec: Vec<f32>,
    pub ranges: Vec<Range<usize>>,
    pub iter_limit: u32,
}

#[derive(Default, Clone)]
pub struct Sequence {
    pub break_reason: BreakReason,
    pub sequence: Vec<PolygonPoint>,
    pub yarn_length: f32,
}

#[derive(Default, Clone)]
pub struct RgbSequence {
    pub r: Sequence,
    pub g: Sequence,
    pub b: Sequence,
    pub black: Sequence,
}

#[derive(Default, Clone)]
pub enum BreakReason {
    #[default]
    EndOfIteration,
    Covered,
    LocalOptimum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolygonPoint(pub usize);

struct ImageBackground {
    /// Total subtracted lightness which to achieve by yarn.
    darkness: f32,
}

pub fn plan(
    image: &GrayImage,
    poly: &Polygon,
    lines: &Lines,
    class: &LineClass,
) -> Result<Sequence, eyre::Report> {
    let (w, h) = image.dimensions();

    let mut draw_points: Vec<_> = poly.points
        .iter()
        .map(|&(x, y)| {
            let x = ((x / 2.0 + 0.5) * w as f32) as i32;
            let y = ((y / 2.0 + 0.5) * h as f32) as i32;
            Point { x, y }
        })
        .collect();

    let bound: imageproc::rect::Rect = {
        let mut r = [0, 0, i32::MAX, i32::MAX];

        for point in &draw_points {
            r[0] = r[0].max(point.x);
            r[1] = r[1].max(point.y);
            r[2] = r[2].min(point.x);
            r[3] = r[3].min(point.y);
        }

        let w = (r[0] - r[2]) as u32;
        let h = (r[1] - r[3]) as u32;
        Rect::at(r[2], r[3]).of_size(w, h)
    };

    let mut mask = GrayImage::new(
        bound.width(),
        bound.height());

    let mut target = GrayImage::new(
        bound.width(),
        bound.height());

    target.copy_from(
        &*image.view(
            bound.left() as u32,
            bound.top() as u32,
            bound.width(),
            bound.height()),
        0, 0
    )?;

    for point in &mut draw_points {
        point.x -= bound.left();
        point.y -= bound.top();
    }

    // Lighten the area we're drawing into.
    imageproc::drawing::draw_polygon_mut(
        &mut mask,
        &draw_points,
        image::Luma([0xff]),
    );

    let analysis = image_background(&mask, &target);

    let mut done = GrayImage::new(
        bound.width(),
        bound.height());

    imageproc::drawing::draw_polygon_mut(
        &mut done,
        &draw_points,
        image::Luma([0xff]),
    );

    let mut current = PolygonPoint(0);

    let mut sequence = Vec::new();
    sequence.push(current);

    let mut yarn_length = 0.0f32;
    let mut break_reason = BreakReason::EndOfIteration;

    let mut xoshiro = Xoshiro128Plus::from_seed({
        let mut seed = [0; 16];
        let deterministic = analysis.darkness.to_ne_bytes();
        seed[..4].copy_from_slice(&deterministic);
        seed
    });

    let mut hit_count = vec![0; lines.ranges.len()];

    for _ in 0..poly.iter_limit {
        if yarn_length >= analysis.darkness * 16.0 {
            break_reason = BreakReason::Covered;
            break;
        }

        let r = &lines.ranges[current.0];
        let threads = &lines.idx_vec[r.start..r.end];
        let weights = &lines.weight_vec[r.start..r.end];

        // Determine the best-fit for the next segment.
        let best_fit = best_fit(
            &mask,
            &target,
            &draw_points,
            current,
            threads,
            lines,
            &done,
            class,
            &mut xoshiro,
            &mut hit_count,
        );

        let Some(best_fit) = best_fit else {
            break_reason = BreakReason::LocalOptimum;
            break;
        };

        let target = threads[best_fit];
        yarn_length += weights[best_fit];

        darken_by_thread(&mut done, &draw_points, current, target);
        hit_count[target.0] += 1;
        sequence.push(target);

        current = target;
    }

    static I: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
    let i = I.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

    mask.save(format!("target/{i}-mask.png"))?;
    done.save(format!("target/{i}.png"))?;
    target.save(format!("target/{i}-target.png"))?;

    Ok(Sequence {
        break_reason,
        sequence,
        yarn_length,
    })
}

impl RgbSequence {
    pub fn channel(&mut self, idx: usize) -> &mut Sequence {
        match idx {
            0 => &mut self.r,
            1 => &mut self.g,
            2 => &mut self.b,
            _ => unreachable!("Not called with this"),
        }
    }
}

fn image_background(
    mask: &GrayImage,
    target: &GrayImage,
) -> ImageBackground {
    let mut darkness = 0.0f32;
    for x in 0..mask.width() {
        for y in 0..mask.height() {
            if *mask.get_pixel(x, y) != image::Luma([0xff]) {
                continue;
            }

            let &image::Luma([t]) = target.get_pixel(x, y);
            darkness += (1.0 - eo_transfer(t)).max(0.0);
        }
    }

    ImageBackground {
        darkness,
    }
}

fn best_fit(
    mask: &GrayImage,
    target: &GrayImage,
    draw_points: &[Point<i32>],
    source: PolygonPoint,
    threads: &[PolygonPoint],
    _: &Lines,
    done: &GrayImage,
    class: &LineClass,
    rng: &mut Xoshiro128Plus,
    hit_count: &mut [u32],
) -> Option<usize> {
    let pre_score = score_img_to_target(mask, target, done);

    let mut scores = Vec::with_capacity(threads.len());
    for (idx, &candidate) in threads.iter().enumerate() {
        let discourage = ((source.0 + candidate.0) % class.of) != class.idx;

        let mut conjecture = done.clone();
        darken_by_thread(&mut conjecture, draw_points, source, candidate);
        let score = score_img_to_target(mask, target, &conjecture);

        if !(score < pre_score) {
            continue;
        }

        let improve = pre_score - score;
        // u from [0; 1)
        let u = (rng.next_u32() as f32) / (2.0f32.powi(32));
        let weight = if discourage {
            u.powf(4.0 / u)
        } else {
            u.powf(1.0 / u)
        };

        scores.push((idx, improve, weight, candidate));
    }

    scores.sort_unstable_by(|(_, a, _, _), (_, b, _, _)| {
        a.total_cmp(b)
    });

    let (best_idx, _, best_w, best_candidate) = scores.pop()?;

    let best_candidate = scores
        .iter()
        .filter(|(_, _, _, candidate)| {
            hit_count[best_candidate.0] > hit_count[candidate.0]
        })
        .max_by(|(_, _, wa, _), (_, _, wb, _)| {
            wa.total_cmp(wb)
        });

    let Some(&(sec_idx, _, sec_w, _)) = best_candidate else {
        return Some(best_idx);
    };

    Some(if best_w > sec_w {
        best_idx
    } else {
        sec_idx
    })
}

fn darken_by_thread(
    image: &mut GrayImage,
    draw_points: &[Point<i32>],
    PolygonPoint(source): PolygonPoint,
    PolygonPoint(target): PolygonPoint,
) {
    type P = image::Luma::<u8>;
    fn interpolate(image::Luma([right]): P, image::Luma([left]): P, left_weight: f32) -> P {
        let w = 0.5f32.powf(left_weight);
        let new = (right as f32 + (left - right) as f32 * w) as u8;

        image::Luma([new])
    }

    imageproc::drawing::draw_antialiased_line_segment_mut(
        image,
        (draw_points[source].x, draw_points[source].y),
        (draw_points[target].x, draw_points[target].y),
        image::Luma([0x00]),
        interpolate);
}

// FIXME: try a perceptual one, in particular for regions which could be filled by ''dithering''.
fn score_img_to_target(
    mask: &GrayImage,
    target: &GrayImage,
    actual: &GrayImage,
) -> f32 {
    assert_eq!(mask.width(), target.width());
    assert_eq!(mask.height(), target.height());
    assert_eq!(actual.width(), target.width());
    assert_eq!(actual.height(), target.height());

    let mut err_sum = 0.0f32;

    for x in (0..mask.width()).step_by(4) {
        for y in (0..mask.height()).step_by(4) {
            let max_x = (x + 4).min(mask.width());
            let max_y = (y + 4).min(mask.height());

            let mut abs_light = 0.0f32;
            let mut perc_light = 0.0f32;
            let mut count = 0;

            for x in x..max_x {
                for y in y..max_y {
                    if *mask.get_pixel(x, y) != image::Luma([0xff]) {
                        continue;
                    }

                    count += 1;
                    let &image::Luma([t]) = target.get_pixel(x, y);
                    let &image::Luma([actual]) = actual.get_pixel(x, y);

                    abs_light += eo_transfer(t);
                    perc_light += eo_transfer(actual);
                }
            }

            if count > 0 {
                err_sum += (abs_light - perc_light).abs() / (count as f32);
            }
        }
    }

    assert!(err_sum.is_finite());
    err_sum
}

pub fn permissible_lines(
    poly: &Polygon,
    (w, h): (u32, u32),
    // 0..1 fraction of rectangle required
    // partial: f32,
) -> Lines {
    let (w, h) = (w as f32 / 2.0, h as f32 / 2.0);

    fn dot(a: (f32, f32), b: (f32, f32)) -> f32 {
        a.0 * b.0 + a.1 * b.1
    }

    fn signed_area(
        poly: &Polygon,
        a: usize,
        mid: usize,
        b: usize,
    ) -> f32 {
        let len = poly.points.len();
        let a = poly.points[a % len];
        let mid = poly.points[mid % len];
        let b = poly.points[b % len];

        let sa = (a.0 - mid.0, a.1 - mid.1);
        let sb = (b.0 - mid.0, b.1 - mid.1);

        sa.0 * sb.1 - sa.1 * sb.0
    }

    let mut lines = Lines::default();
    lines.iter_limit = poly.iter_limit;

    let len = poly.points.len();
    for (offset, _) in poly.points.iter().enumerate() {
        // Find those targets for which the line.
        let offset = offset + len;
        let count = poly.points.len() - 3;
        let start = lines.idx_vec.len();

        for candidate in (offset+1..).skip(1).take(count) {
            let prior = candidate - 1;
            let post = candidate + 1;

            if signed_area(poly, prior, offset, candidate) > 0.0 || signed_area(poly, post, offset, candidate) < 0.0 {
                continue;
            }


            let a = poly.points[candidate % len];
            let start = poly.points[offset % len];
            let sa = ((a.0 - start.0)*w, (a.1 - start.1)*h);
            let length = dot(sa, sa).sqrt();

            lines.idx_vec.push(PolygonPoint(candidate % len));
            lines.weight_vec.push(length);
        }

        let end = lines.idx_vec.len();
        lines.ranges.push(start..end);
    }

    lines
}
