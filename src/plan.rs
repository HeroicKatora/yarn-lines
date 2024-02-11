use std::ops::Range;

use image::{GenericImage, GenericImageView, GrayImage};
use imageproc::{rect::Rect, point::Point};

use crate::poly::Polygon;

#[derive(Default)]
pub struct Lines {
    pub idx_vec: Vec<PolygonPoint>,
    pub ranges: Vec<Range<usize>>,
    pub sequence: Vec<PolygonPoint>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolygonPoint(pub usize);

pub fn plan(
    image: &GrayImage,
    poly: &Polygon,
    lines: &mut Lines,
) -> Result<(), eyre::Report> {
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

    let background = image_background(&mask, &target);

    let mut done = GrayImage::new(
        bound.width(),
        bound.height());

    imageproc::drawing::draw_polygon_mut(
        &mut done,
        &draw_points,
        background,
    );

    let mut current = PolygonPoint(0);
    lines.sequence.push(current);

    for _ in 0..128 {
        let r = &lines.ranges[current.0];
        let threads = &lines.idx_vec[r.start..r.end];

        // Determine the best-fit for the next segment.
        let best_fit = best_fit(
            &mask,
            &target,
            &draw_points,
            current,
            threads,
            &lines,
            &done,
        );

        let target = lines.idx_vec[r.start..r.end][best_fit];
        darken_by_thread(&mut done, &draw_points, current, target);

        lines.sequence.push(target);
        current = target;
    }

    static I: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
    let i = I.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

    mask.save(format!("target/{i}-mask.png"))?;
    done.save(format!("target/{i}.png"))?;
    target.save(format!("target/{i}-target.png"))?;

    Ok(())
}

fn image_background(
    mask: &GrayImage,
    target: &GrayImage,
) -> image::Luma<u8> {
    let mut abs_light = 0.0f32;
    let mut count = 0;

    for x in 0..mask.width() {
        for y in 0..mask.height() {
            if *mask.get_pixel(x, y) != image::Luma([0xff]) {
                continue;
            }

            count += 1;
            let &image::Luma([t]) = target.get_pixel(x, y);
            abs_light += (t as f32 / 255.).powf(2.2);
        }
    }

    image::Luma([if count == 0 {
        0xff
    } else {
        let med = abs_light / (count as f32);
        (med.powf(1./2.2) * 255.) as u8
    }])
}

fn best_fit(
    mask: &GrayImage,
    target: &GrayImage,
    draw_points: &[Point<i32>],
    source: PolygonPoint,
    threads: &[PolygonPoint],
    lines: &Lines,
    done: &GrayImage,
) -> usize {
    let _pre_score = score_img_to_target(mask, target, done);

    let mut scores = Vec::with_capacity(threads.len());
    for &candidate in threads {
        let mut conjecture = done.clone();
        darken_by_thread(&mut conjecture, draw_points, source, candidate);
        let score = score_img_to_target(mask, target, &conjecture);
        scores.push(score);
    }

    let best = scores
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            a.total_cmp(b)
        })
        .map(|(idx, _)| idx)
        .unwrap();

    best
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

                    abs_light += (t as f32 / 255.).powf(2.2);
                    perc_light += (actual as f32 / 255.).powf(2.2);
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
    // 0..1 fraction of rectangle required
    // partial: f32,
) -> Lines {
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

    for (offset, _) in poly.points.iter().enumerate() {
        // Find those targets for which the line.
        let offset = offset + poly.points.len();
        let count = poly.points.len() - 3;
        let start = lines.idx_vec.len();

        for candidate in (offset+1..).skip(1).take(count) {
            let prior = candidate - 1;
            let post = candidate + 1;

            if signed_area(poly, prior, offset, candidate) > 0.0 || signed_area(poly, post, offset, candidate) < 0.0 {
                continue;
            }

            lines.idx_vec.push(PolygonPoint(candidate % poly.points.len()));
        }

        let end = lines.idx_vec.len();
        lines.ranges.push(start..end);
    }

    lines
}
