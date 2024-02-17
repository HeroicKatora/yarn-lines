//! Turn a definition file into polygons.
use serde::Deserialize;
use crate::color;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Definition {
    Circles {
        circles: Vec<Circle>,
        #[serde(default)]
        primaries: Primaries,
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
    /// How much to split each segment along the circle.
    #[serde(default)]
    pub split_per_segment: u32,
    /// How much to split each segment along the circle.
    #[serde(default)]
    pub split_per_segment_inner: u32,
    /// The number of separators between this and the last circle.
    pub window_split: u32,
    /// Point offset at which the windows begins.
    pub offset: u32,
    /// Point offset at which the windows begins, in the inner.
    pub offset_inner: u32,
    #[serde(default = "default_iter_limit")]
    pub iter_limit: u32,
}

#[derive(Deserialize, Debug)]
pub struct Primaries {
    yarn0: [u8; 3],
    yarn1: [u8; 3],
    yarn2: [u8; 3],
}

fn default_iter_limit() -> u32 {
    512
}

#[derive(Debug)]
pub struct Polygons {
    pub windows: Vec<Polygon>,
    pub primaries: Primaries,
}

#[derive(Debug)]
pub struct Polygon {
    pub points: Vec<(f32, f32)>,
    pub iter_limit: u32,
    pub names: Vec<String>,
}

pub fn read(def: impl std::io::Read) -> Result<Polygons, eyre::Report> {
    let def: Definition = serde_json::from_reader(def)?;
    let Definition::Circles { mut circles, primaries } = def;

    let middle = Circle {
        radius: 0.0,
        points_on_circle: 1,
        windows: 0,
        split_per_segment: 0,
        split_per_segment_inner: 0,
        window_split: 0,
        offset: 0,
        offset_inner: 0,
        iter_limit: default_iter_limit(),
    };

    circles.insert(0, middle);

    let mut windows = vec![];
    for slice in circles.windows(2) {
        let &[ref pre, ref post] = slice.try_into().unwrap();
        append_windows(&mut windows, pre, post)?;
    }

    Ok(Polygons {
        windows,
        primaries,
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

    fn window_idx(idx: u32, circle: &Circle, post: &Circle) -> u32 {
        if (circle as *const _ as usize) == (post as *const _ as usize) {
            post.offset + (idx * circle.points_on_circle) / post.windows
        } else {
            post.offset_inner + (idx * circle.points_on_circle) / post.windows
        }
    }

    let c_pre = 2.0 * std::f32::consts::PI / pre.points_on_circle as f32;
    let c_post = 2.0 * std::f32::consts::PI / post.points_on_circle as f32;

    for idx in 0..post.windows {
        let mut points = vec![];
        let mut names = vec![];

        let outer_iter = window_idx(idx, post, post)..window_idx(idx+1, post, post);
        for (idx, o) in outer_iter.enumerate() {
            let a = point_by_idx(o, c_post, post.radius);
            points.push(a);
            names.push(format!("o{idx}"));

            for mid in 1..post.split_per_segment {
                let b = point_by_idx(o + 1, c_post, post.radius);
                let f = mid as f32 / post.split_per_segment as f32;
                points.push(lerp(a, b, f));
                names.push(format!("o{idx}.{mid}"));
            }
        }

        points.push(point_by_idx(window_idx(idx+1, post, post), c_post, post.radius));
        let post_name = (window_idx(idx, post, post)..window_idx(idx+1, post, post)).len();
        names.push(format!("o{post_name}"));

        {
            let a = window_idx(idx+1, post, post);
            let b = window_idx(idx+1, pre, post);

            let a = point_by_idx(a, c_post, post.radius);
            let b = point_by_idx(b, c_pre, pre.radius);

            for mid in 1..post.window_split {
                let f = mid as f32 / post.window_split as f32;
                points.push(lerp(a, b, f));
                names.push(format!("r.{mid}"));
            }
        }

        let inner_iter = ((1 + window_idx(idx, pre, post))..=window_idx(idx+1, pre, post)).rev();
        for (idx, o) in inner_iter.enumerate() {
            let a = point_by_idx(o, c_pre, pre.radius);
            points.push(a);
            names.push(format!("o{idx}"));

            for mid in 1..post.split_per_segment_inner {
                let b = point_by_idx(o - 1, c_pre, pre.radius);
                let f = mid as f32 / post.split_per_segment_inner as f32;
                points.push(lerp(a, b, f));
                names.push(format!("i{idx}.{mid}"));
            }
        }

        points.push(point_by_idx(window_idx(idx, pre, post), c_pre, pre.radius));
        let post_name = (window_idx(idx, pre, post)..window_idx(idx+1, pre, post)).len();
        names.push(format!("i{post_name}"));

        {
            let a = window_idx(idx, pre, post);
            let b = window_idx(idx, post, post);

            let a = point_by_idx(a, c_pre, pre.radius);
            let b = point_by_idx(b, c_post, post.radius);

            for mid in 1..post.window_split {
                let f = mid as f32 / post.window_split as f32;
                points.push(lerp(a, b, f));
                names.push(format!("l.{mid}"));
            }
        }

        windows.push(Polygon {
            points,
            iter_limit: post.iter_limit,
            names,
        });
    }

    Ok(())
}

impl Default for Primaries {
    fn default() -> Self {
        Primaries {
            yarn0: [0xd2, 0x85, 0x2b],
            // green: image::Rgb([0xe6, 0xff, 0xff]),
            yarn1: [0xff, 0xff, 0xff],
            yarn2: [0xd2, 0xe2, 0xef],
        }
    }
}

impl Primaries {
    pub fn to_color_base(&self) -> color::PrimaryBase {
        color::PrimaryBase {
            red: image::Rgb(self.yarn0),
            green: image::Rgb(self.yarn1),
            blue: image::Rgb(self.yarn2),
        }
    }
}
