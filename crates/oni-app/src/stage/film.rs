//! The film over the largest gift: one photo as the app rendered it on the neutral film, its negative on the first
//! stock and its print on three (`examples/welcome_stills`), and what the film does in each step of its page.

use super::ease::{glide, land, mix, span};

/// The photo and its negative: the welcome's own files.
const PICTURES: [&[u8]; 2] = [include_bytes!("../../../oni-ui/ui/welcome/film-photo.jpg"), include_bytes!("../../../oni-ui/ui/welcome/film-negative.jpg")];
/// Its prints, the first on the negative's own stock.
const PRINTS: [&[u8]; 3] = [
    include_bytes!("../../../oni-ui/ui/welcome/film-print-1.jpg"),
    include_bytes!("../../../oni-ui/ui/welcome/film-print-2.jpg"),
    include_bytes!("../../../oni-ui/ui/welcome/film-print-3.jpg"),
];
/// The stocks the prints were made on, in their order: the keys of their virtual films.
pub const STOCKS: [&str; 3] = ["kodak-ektar-100", "fujifilm-velvia-50", "kodak-tri-x-400"];

/// The steps the huge gift's page tells, in order, and how long each plays before the next; the last holds.
pub const STEPS: usize = 5;
const LENGTHS: [f32; STEPS] = [1.9, 2.8, 5.2, 3.2, f32::INFINITY];
/// In the last step: when the print has come forward, when the second stock comes over it, and while the photo
/// lies back over half of that one.
pub const PRINTED: f32 = 2.4;
pub const SECOND: f32 = 4.2;
pub const COMPARE: (f32, f32) = (5.2, 8.6);
/// Then the third stock, and the three side by side, which holds.
pub const THIRD: f32 = 9.2;
pub const FINALE: f32 = 11.4;

pub fn length(step: usize) -> f32 {
    LENGTHS[step.min(STEPS - 1)]
}

/// A JPEG's pixels as RGBA with its size; a file that cannot be read is one black pixel.
fn decoded(jpeg: &[u8]) -> (Vec<u8>, u32, u32) {
    let mut decoder = jpeg_decoder::Decoder::new(jpeg);
    let rgb = decoder.decode().ok().zip(decoder.info()).filter(|(_, info)| info.pixel_format == jpeg_decoder::PixelFormat::RGB24);
    match rgb {
        Some((rgb, info)) => (rgb.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(), u32::from(info.width), u32::from(info.height)),
        None => (vec![0, 0, 0, 255], 1, 1),
    }
}

fn texture(device: &wgpu::Device, queue: &wgpu::Queue, pixels: &[u8], width: u32, height: u32) -> wgpu::TextureView {
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("stage picture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 4), rows_per_image: Some(height) };
    queue.write_texture(texture.as_image_copy(), pixels, layout, size);
    texture.create_view(&Default::default())
}

/// The plate's three pictures: the photo, the negative, and the prints one over the other in one picture.
pub fn pictures(device: &wgpu::Device, queue: &wgpu::Queue) -> [wgpu::TextureView; 3] {
    let [photo, negative] = PICTURES.map(|jpeg| {
        let (pixels, width, height) = decoded(jpeg);
        texture(device, queue, &pixels, width, height)
    });
    let prints = PRINTS.map(decoded);
    let (width, height) = (prints[0].1, prints[0].2);
    // Prints of another size than the first would shear: they are rendered from one photo at one size.
    let stacked: Vec<u8> = prints.iter().flat_map(|(pixels, w, h)| if (*w, *h) == (width, height) { pixels.clone() } else { vec![0; (width * height * 4) as usize] }).collect();
    [photo, negative, texture(device, queue, &stacked, width, height * PRINTS.len() as u32)]
}

/// The film as a step shows it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Plate {
    /// How far the photo has risen out of the box, 0 inside to 1 at its place.
    pub risen: f32,
    /// The print's sheet and the strip of film behind it, each 0 away to 1 there; how far the strip has to run.
    pub sheet: f32,
    pub strip: f32,
    pub slide: f32,
    pub light: f32,
    /// The latent picture the exposure leaves, and the film developed, each 0 to 1.
    pub latent: f32,
    pub developed: f32,
    /// The grain and the halation on the film, 0 none to 1 their dose, and whether the grain still shimmers.
    pub grain: f32,
    pub halation: f32,
    pub shimmer: bool,
    /// The scanner's bar across the frame, 0 to 1 across it, its lamp, and the film inverted, 0 to 1.
    pub head: f32,
    pub lamp: f32,
    pub inverted: f32,
    /// How close the camera stands, 0 the whole strip to 1 its grain; how far the finished print has come forward.
    pub close: f32,
    pub forward: f32,
    /// Which print shows, the share past a whole number the next stock coming over it; the three fanned out.
    pub stock: f32,
    pub fan: f32,
}

/// The film at `step`, `t` seconds in; each step starts where the one before ends.
pub fn plate(step: usize, t: f32) -> Plate {
    let photo = Plate { risen: 1.0, sheet: 1.0, light: 1.0, head: -1.0, ..Plate::default() };
    let film = Plate { sheet: 0.0, strip: 1.0, latent: 1.0, ..photo };
    let negative = Plate { developed: 1.0, grain: 1.0, halation: 1.0, ..film };
    match step {
        // The photo rises out of the box into the camera's sight, its own light coming up as it clears the rim.
        0 => Plate { risen: land(span(t, 0.0, 1.6)), sheet: glide(span(t, 0.0, 0.4)), light: glide(span(t, 0.2, 1.3)), ..photo },
        // The strip runs in behind the photo and stops, its gate about it; a flash, as a shutter's, and the photo
        // is gone into the film: a latent picture, faint.
        1 => {
            let flash = if t < 1.6 { glide(span(t, 1.5, 1.6)) } else { (-(t - 1.6) / 0.2).exp() };
            Plate { sheet: 1.0 - glide(span(t, 1.6, 2.0)), light: 1.0 + 2.2 * flash, strip: span(t, 0.2, 0.25), slide: 15.0 * (1.0 - land(span(t, 0.2, 1.2))), latent: span(t, 1.55, 1.65), ..film }
        }
        // The picture comes up in the developer, the base turning orange about it; then the camera comes to the
        // lighthouse, where the grain settles and the halation blooms about the lights; then back.
        2 => Plate {
            developed: span(t, 0.2, 1.9),
            close: glide(span(t, 1.9, 2.7)) - glide(span(t, 4.3, 5.1)),
            grain: glide(span(t, 2.4, 3.3)),
            shimmer: t < 3.7,
            halation: glide(span(t, 3.0, 3.7)),
            ..film
        },
        // The lamp comes on behind the film and the scanner's bar crosses it at one speed: behind it the film is
        // read, lit through and its orange gone.
        3 => Plate { lamp: glide(span(t, 0.0, 0.4)), head: mix(-0.9, 1.9, span(t, 0.4, 2.9)), ..negative },
        // Every tone crosses to its opposite; the strip falls away and the print comes forward. Then the second
        // stock, the photo laid back over it a while, the third, and the three side by side.
        _ => Plate {
            lamp: 1.0,
            head: 2.0,
            inverted: glide(span(t, 0.3, 1.3)),
            strip: 1.0 - glide(span(t, 1.3, 2.0)),
            sheet: span(t, 1.3, 1.4),
            forward: glide(span(t, 1.2, PRINTED)),
            stock: glide(span(t, SECOND, SECOND + 0.6)) + glide(span(t, THIRD, THIRD + 0.6)) + glide(span(t, FINALE, FINALE + 0.7)),
            fan: glide(span(t, FINALE, FINALE + 1.6)),
            ..negative
        },
    }
}
