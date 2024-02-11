//! Turn a definition file into polygons.
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "kind")]
pub enum Definition {
    Circles {
        circles: Vec<Circle>,
    }
}

#[derive(Deserialize)]
pub struct Circle {
    /// The radius at which to place base points.
    pub radius: f32,
    /// The number of points to place on that circle.
    pub points_on_circle: u32,
    /// The number of windows to place within that circle.
    pub windows: u32,
    /// The number of separators between this and the last circle.
    pub window_split: u32,
    /// Point offset at which the windows begins.
    pub offset: u32,
    /// Point offset at which the windows begins, in the inner.
    pub offset_inner: u32,
}

#[derive(Debug)]
pub struct Polygons {
    pub windows: Vec<Polygon>,
}

#[derive(Debug)]
pub struct Polygon {
    points: Vec<(f32, f32)>,
}

pub fn read(def: impl std::io::Read) -> Result<Polygons, eyre::Report> {
    let def: Definition = serde_json::from_reader(def)?;
    let Definition::Circles { mut circles } = def;

    let middle = Circle {
        radius: 0.0,
        points_on_circle: 1,
        windows: 0,
        window_split: 0,
        offset: 0,
        offset_inner: 0,
    };

    circles.insert(0, middle);

    let mut windows = vec![];
    for slice in circles.windows(2) {
        let &[ref pre, ref post] = slice.try_into().unwrap();
        append_windows(&mut windows, pre, post)?;
    }

    Ok(Polygons {
        windows,
    })
}

fn append_windows(windows: &mut Vec<Polygon>, pre: &Circle, post: &Circle) -> Result<(), eyre::Report> {
    fn point_by_idx(idx: u32, c: f32, radius: f32) -> (f32, f32) {
        let angle = idx as f32 * c;
        let (s, c) = angle.sin_cos();
        (s * radius, c * radius)
    }

    fn lerp(a: (f32, f32), b: (f32, f32), f: f32) -> (f32, f32) {
        // This is shit, but good enough.
        fn lerp(x: f32, y: f32, f: f32) -> f32 {
            (y - x) * f + x
        }

        (
            lerp(a.0, b.0, f),
            lerp(a.1, b.1, f),
        )
    }

    let step_pre = pre.points_on_circle / post.windows;
    let step_post = post.points_on_circle / post.windows;

    let c_pre = 2.0 * std::f32::consts::PI / pre.points_on_circle as f32;
    let c_post = 2.0 * std::f32::consts::PI / post.points_on_circle as f32;

    for idx in 0..post.windows {
        let mut points = vec![];

        for o in (post.offset + idx * step_post)..=(post.offset + (idx+1) * step_post) {
            points.push(point_by_idx(o, c_post, post.radius));
        }

        {
            let a = post.offset + (idx+1) * step_post;
            let b = post.offset_inner + (idx+1) * step_pre;

            let a = point_by_idx(a, c_post, post.radius);
            let b = point_by_idx(b, c_pre, pre.radius);

            for mid in 1..post.window_split {
                let f = mid as f32 / post.window_split as f32;
                points.push(lerp(a, b, f))
            }
        }

        for o in ((post.offset_inner + idx * step_pre)..=(post.offset_inner + (idx+1) * step_pre)).rev() {
            points.push(point_by_idx(o, c_pre, pre.radius));
        }

        {
            let a = post.offset_inner + idx * step_pre;
            let b = post.offset + idx * step_post;

            let a = point_by_idx(a, c_pre, pre.radius);
            let b = point_by_idx(b, c_post, post.radius);

            for mid in 1..post.window_split {
                let f = mid as f32 / post.window_split as f32;
                points.push(lerp(a, b, f))
            }
        }

        windows.push(Polygon {
            points,
        });
    }

    Ok(())
}
