use image::{GrayImage, RgbImage};
use super::{eo_transfer, oe_transfer};

pub struct PrimaryBase {
    pub red: image::Rgb::<u8>,
    pub green: image::Rgb::<u8>,
    pub blue: image::Rgb::<u8>,
}

#[derive(Debug)]
struct LabBase {
    pub red: Lab,
    pub green: Lab,
    pub blue: Lab,

    pub rg_base: [[f32; 2]; 2],
    pub gb_base: [[f32; 2]; 2],
    pub br_base: [[f32; 2]; 2],
}

pub struct ColorPlan {
    pub gray: GrayImage,
    pub red: GrayImage,
    pub green: GrayImage,
    pub blue: GrayImage,
}

pub fn decouple(
    image: &RgbImage,
    primaries: &PrimaryBase,
) -> ColorPlan {
    let (w, h) = image.dimensions();

    let base = {
        let red = image_rgb_to_lab(primaries.red);
        let green = image_rgb_to_lab(primaries.green);
        let blue = image_rgb_to_lab(primaries.blue);

        LabBase {
            rg_base: inverter(red, green),
            gb_base: inverter(green, blue),
            br_base: inverter(blue, red),
            red,
            green,
            blue,
        }
    };

    eprintln!("{base:?}");

    let lab: Vec<Primaries> = image
        .pixels()
        .map(|c: &image::Rgb::<u8>| {
            image_rgb_to_lab(*c)
        })
        .map(|lab| {
            decouple_pixel(lab, &base)
        })
        .collect();

    let gray: Vec<u8> = lab
        .iter()
        .map(|&lab| oe_transfer(lab.0[0].max(0.0).min(1.0)))
        .collect();

    let red: Vec<u8> = lab
        .iter()
        .map(|&lab| oe_transfer(1.0 - lab.0[1].max(0.0).min(1.0)))
        .collect();

    let green: Vec<u8> = lab
        .iter()
        .map(|&lab| oe_transfer(1.0 - lab.0[2].max(0.0).min(1.0)))
        .collect();

    let blue: Vec<u8> = lab
        .iter()
        .map(|&lab| oe_transfer(1.0 - lab.0[3].max(0.0).min(1.0)))
        .collect();

    ColorPlan {
        gray: GrayImage::from_raw(w, h, gray).unwrap(),
        red: GrayImage::from_raw(w, h, red).unwrap(),
        green: GrayImage::from_raw(w, h, green).unwrap(),
        blue: GrayImage::from_raw(w, h, blue).unwrap(),
    }
}

#[derive(Clone, Copy, Debug)]
struct Lab([f32; 3]);

#[derive(Clone, Copy)]
struct LinearRgb([f32; 3]);

#[derive(Clone, Copy, Debug)]
struct Primaries([f32; 4]);

fn image_rgb_to_lab(c: image::Rgb::<u8>) -> Lab {
    let image::Rgb([r, g, b]) = c;

    let c = LinearRgb([
        eo_transfer(r),
        eo_transfer(g),
        eo_transfer(b),
    ]);

    linear_srgb_to_oklab(c)
}

fn linear_srgb_to_oklab(c: LinearRgb) -> Lab {
    let LinearRgb([r, g, b]) = c;

    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
	let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
	let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

    let l_ = cbrtf(l);
    let m_ = cbrtf(m);
    let s_ = cbrtf(s);

    Lab([
        0.2104542553*l_ + 0.7936177850*m_ - 0.0040720468*s_,
        1.9779984951*l_ - 2.4285922050*m_ + 0.4505937099*s_,
        0.0259040371*l_ + 0.7827717662*m_ - 0.8086757660*s_,
    ])
}

fn oklab_to_linear_srgb(c: Lab) -> LinearRgb {
    let Lab([l, a, b]) = c;

    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;

    let l = l_*l_*l_;
    let m = m_*m_*m_;
    let s = s_*s_*s_;

    LinearRgb([
		 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
		-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
		-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ])
}

fn cbrtf(v: f32) -> f32 {
    v.powf(1.0/3.0)
}

fn inverter(
    ma: Lab,
    mb: Lab,
) -> [[f32; 2]; 2] {
    let [_, a, b] = ma.0;
    let [_, c, d] = mb.0;
    let det = a * d - c * b;

    [
        [d / det, -b / det],
        [-c / det, a / det],
    ]
}

fn decouple_pixel(lab: Lab, base: &LabBase) -> Primaries {
    fn invfactors(lab: Lab, base: &[[f32; 2]; 2]) -> [f32; 2] {
        let [_, a, b] = lab.0;
        let ia = a * base[0][0] + b * base[0][1];
        let ib = a * base[1][0] + b * base[1][1];
        [ia, ib]
    }

    fn primaries_of(l: f32, a: f32, b: f32, i0: usize, i1: usize) -> Primaries {
        let a = a.max(0.0).min(1.0);
        let b = b.max(0.0).min(1.0);
        let primary_sum = a + b;
        let coefficient = primary_sum.min(1.0);
        // Grayness coefficient, such that the mix will be right.
        let g = 1.0 - coefficient;

        let mut f = [g * l, 0.0, 0.0, 0.0];

        if coefficient > 0.01 {
            f[i0] = a / coefficient;
            f[i1] = b / coefficient;
        }

        Primaries(f)
    }

    let l = lab.0[0];

    let [a, b] = invfactors(lab, &base.br_base);
    if a >= 0.0 && b >= 0.0 {
        return primaries_of(l, a, b, 3, 1);
    }

    let [a, b] = invfactors(lab, &base.rg_base);
    if a >= 0.0 && b >= 0.0 {
        return primaries_of(l, a, b, 1, 2);
    }

    let [a, b] = invfactors(lab, &base.gb_base);
    if a >= 0.0 && b >= 0.0 {
        return primaries_of(l, a, b, 2, 3);
    }

    return Primaries([l, 0.0, 0.0, 0.0])
}
