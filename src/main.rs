use ansi_term::Color::Green;
use ansi_term::{Color::Red, Style};
use clap::Parser;
use image::{Rgb, RgbImage};
use rand::RngExt;
use spinners::{Spinner, Spinners};
use std::cmp::max;

/// Procedural island generator
#[derive(Parser, Debug)]
struct Args {
    /// Image size (minimum: 16x16, maximum: 1024x1024, recommended: 128x128)
    #[arg(short, long, default_value = "128x128")]
    size: String,

    /// Output PNG image
    #[arg(short, long, default_value = "output.png")]
    output: String,

    /// Upscale image by a multiplier
    #[arg(long)]
    upscale: Option<u8>,
}

const NEIGHBOR_OFFSETS: [(isize, isize); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

#[derive(Clone)]
struct Image {
    width: u16,
    height: u16,
    data: Vec<u8>,
}

impl Image {
    fn new(width: u16, height: u16) -> Self {
        let rng = rand::rng();
        Self {
            width,
            height,
            data: rng
                .random_iter()
                .take(width as usize * height as usize)
                .collect(),
        }
    }

    #[inline]
    fn get(&self, x: u16, y: u16) -> u8 {
        self.data[y as usize * self.width as usize + x as usize]
    }

    #[inline]
    fn set(&mut self, x: u16, y: u16, value: u8) {
        self.data[y as usize * self.width as usize + x as usize] = value;
    }
}

fn main() {
    let args = Args::parse();
    let size = args.size.split('x').collect::<Vec<&str>>();
    if size.len() != 2
        || size.iter().any(|s| {
            let v = s.parse::<usize>();
            v.is_err() || v.clone().unwrap() > 1024 || v.clone().unwrap() < 16
        })
    {
        eprintln!(
            "{}",
            Red.paint(format!("Invalid size provided: {}", args.size))
        );
        std::process::exit(1)
    }
    let width: u16 = size[0].parse().unwrap();
    let height: u16 = size[1].parse().unwrap();
    println!(
        "Size: {}",
        Style::new().bold().paint(format!("{width}x{height}"))
    );
    let output: String = if !args.output.ends_with(".png") {
        let mut n = args.output.clone();
        n.push_str(".png");
        n
    } else {
        args.output
    };
    println!("Output: {}", Style::new().bold().paint(output.clone()));
    let scale = args.upscale;
    if scale.is_some() {
        println!(
            "Upscaling: {}x",
            Style::new()
                .bold()
                .paint(scale.unwrap().clone().to_string())
        );
    }

    let mut rng = rand::rng();

    let mut pixels = Image::new(width, height);

    let mut spinner = Spinner::with_timer(Spinners::Dots, "Generating noise...".into());
    // initial noise generation
    for l in 0..=12 {
        for x in 0..width {
            for y in 0..height {
                if l == 0 {
                    pixels.set(x, y, max(0, (pixels.get(x, y) as i16 - 128) as u8));
                    continue;
                }
                let surrounding = get_surrounding_pixels(&pixels, x, y);
                let mut sum: u32 = surrounding.iter().map(|&i| i as u32).sum();
                sum += pixels.get(x, y) as u32;

                let mut average = (sum / (surrounding.len() as u32 + 1)) as u8;
                if l <= 10 {
                    // influencing to make a rougher division between land and water
                    if average >= 126 {
                        average += (255 - average) / 2;
                    } else if l < 4 || average <= 64 {
                        average /= 3
                    }
                }
                if l == 10 {
                    // cutting water and land completely, but still leaving 2 loops to smooth things out
                    if average >= 192 {
                        average = 255
                    } else {
                        average = 0
                    }
                }
                pixels.set(x, y, average);
            }
        }
    }
    spinner.stop_and_persist("🗸", "Noise generated".into());

    let mut spinner = Spinner::with_timer(
        Spinners::Dots,
        "Generating image (might take a while)...".into(),
    );

    let mut img = RgbImage::new(width as u32, height as u32);

    let mut light_blue_pixels: Vec<(u16, u16)> = Vec::new();
    let mut grass_pixels: Vec<(u16, u16)> = Vec::new();
    let mut dark_sand_pixels: Vec<(u16, u16)> = Vec::new();
    for x in 0..width {
        for y in 0..height {
            let pixel = pixels.get(x, y);
            if pixel >= 208 {
                // grass
                let surrounding_pixels = get_surrounding_pixels(&pixels, x, y);
                let surrounding_pos = get_surrounding_pixels_positions(&pixels, x, y);
                // random grass
                if !surrounding_pixels.iter().any(|p| p < &208)
                    && grass_pixels
                        .iter()
                        .filter(|p| surrounding_pos.contains(*p))
                        .collect::<Vec<_>>()
                        .is_empty()
                    && rng.random_range(0..=10) == 1
                {
                    img.put_pixel(x as u32, y as u32, Rgb([64, 172, 109]));
                    grass_pixels.push((x, y));
                } else if !grass_pixels.iter().any(|p| *p == (x, y)) {
                    img.put_pixel(x as u32, y as u32, Rgb([53, 196, 112]));
                }
            } else if pixel >= 78      // sand
                && get_surrounding_pixels(&pixels, x, y)
                .iter()
                .filter(|p| p >= &&208)
                .collect::<Vec<_>>()
                .len()
                <= 8
            {
                if !grass_pixels.iter().any(|p| *p == (x, y)) {
                    let surrounding_pixels = get_surrounding_pixels(&pixels, x, y);
                    // sand in contact with grass
                    if surrounding_pixels.iter().any(|p| p >= &208) {
                        img.put_pixel(x as u32, y as u32, Rgb([242, 225, 49]));
                        dark_sand_pixels.push((x, y));
                    } else {
                        img.put_pixel(x as u32, y as u32, Rgb([232, 215, 39]));
                    }
                }
            } else {
                // water
                let surrounding_pixels = get_surrounding_pixels(&pixels, x, y);
                let surrounding_pos = get_surrounding_pixels_positions(&pixels, x, y);
                if surrounding_pixels.iter().any(|p| p >= &78) {
                    // shore water
                    img.put_pixel(x as u32, y as u32, Rgb([91, 151, 247]));
                    light_blue_pixels.push((x, y));
                } else if surrounding_pos // medium-depth water
                    .iter()
                    .filter(|p| light_blue_pixels.contains(p))
                    .collect::<Vec<_>>()
                    .is_empty()
                {
                    img.put_pixel(x as u32, y as u32, Rgb([19, 93, 212]));
                } else {
                    // deep water
                    img.put_pixel(x as u32, y as u32, Rgb([39, 113, 232]));
                }
            }
        }
    }
    spinner.stop_and_persist("🗸", "Image generated".into());

    match img.save(output.clone()) {
        Ok(_) => {
            println!("{}", Green.bold().paint("Image saved!"))
        }
        Err(e) => {
            println!("{} {e}", Red.bold().paint("Could not save image:"))
        }
    };

    if scale.is_some() {
        let mut spinner = Spinner::with_timer(Spinners::Dots, "Upscaling image...".into());

        let scale = scale.unwrap();

        let mut upscaled_img =
            RgbImage::new(img.width() * scale as u32, img.height() * scale as u32);

        for x in 0..img.width() {
            let x_pos = x * scale as u32;
            for y in 0..img.height() {
                let y_pos = y * scale as u32;
                for scale_y in 0..scale {
                    for scale_x in 0..scale {
                        upscaled_img.put_pixel(
                            x_pos + scale_x as u32,
                            y_pos + scale_y as u32,
                            *img.get_pixel(x, y),
                        );
                    }
                }
            }
        }

        spinner.stop_and_persist("🗸", "Upscaled image generated".into());

        let mut output = output.strip_suffix(".png").unwrap().to_string();
        output.push_str(format!("-{scale}x.png").as_str());
        match upscaled_img.save(output.clone()) {
            Ok(_) => {
                println!("{}", Green.bold().paint("Upscaled image saved!"))
            }
            Err(e) => {
                println!("{} {e}", Red.bold().paint("Could not save image:"))
            }
        };
    }
}

fn get_surrounding_pixels(image: &Image, x: u16, y: u16) -> Vec<u8> {
    NEIGHBOR_OFFSETS
        .iter()
        .filter_map(|&(dx, dy)| {
            let nx = x.checked_add_signed(dx as i16)?;
            let ny = y.checked_add_signed(dy as i16)?;

            if nx < image.width && ny < image.height {
                Some(image.get(nx, ny).clone())
            } else {
                None
            }
        })
        .collect()
}

fn get_surrounding_pixels_positions(image: &Image, x: u16, y: u16) -> Vec<(u16, u16)> {
    NEIGHBOR_OFFSETS
        .iter()
        .filter_map(|&(dx, dy)| {
            let nx = x.checked_add_signed(dx as i16)?;
            let ny = y.checked_add_signed(dy as i16)?;

            if nx < image.width && ny < image.height {
                Some((nx, ny))
            } else {
                None
            }
        })
        .collect()
}
