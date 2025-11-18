use anyhow::{Result, anyhow};
use tray_icon::Icon;

pub fn create_icon(color: (u8, u8, u8)) -> Result<Icon> {
    let size = 22;
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let (r, g, b) = color;

    let draw_pixel = |pixels: &mut [u8], x: i32, y: i32, alpha: u8| {
        if x >= 0 && x < size && y >= 0 && y < size {
            let idx = ((y * size + x) * 4) as usize;
            pixels[idx] = r;
            pixels[idx + 1] = g;
            pixels[idx + 2] = b;
            pixels[idx + 3] = alpha;
        }
    };

    for angle in 0..360 {
        let rad = (angle as f32).to_radians();
        let x = 11 + (7.0 * rad.cos()) as i32;
        let y = 11 + (7.0 * rad.sin()) as i32;
        draw_pixel(&mut pixels, x, y, 255);
    }

    for dy in -5..=5 {
        for dx in -5..=5 {
            if dx * dx + dy * dy <= 25 {
                draw_pixel(&mut pixels, 11 + dx, 11 + dy, 180);
            }
        }
    }

    for dy in -2..=2 {
        for dx in -2..=2 {
            if dx * dx + dy * dy <= 4 {
                draw_pixel(&mut pixels, 11 + dx, 11 + dy, 255);
            }
        }
    }

    for i in 0..3 {
        draw_pixel(&mut pixels, 11, 3 + i, 255);
        draw_pixel(&mut pixels, 11, 16 + i, 255);
    }

    Icon::from_rgba(pixels, size as u32, size as u32)
        .map_err(|err| anyhow!("failed to build icon: {err}"))
}
