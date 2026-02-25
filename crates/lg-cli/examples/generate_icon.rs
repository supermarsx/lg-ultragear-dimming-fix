//! Programmatic ICO generator for lg-ultragear-dimming-fix.
//!
//! Draws a stylised LCD monitor icon at 16×16, 32×32, 48×48, and 256×256.
//! The design: a rounded monitor bezel (dark charcoal) with a gradient
//! screen showing a subtle color-calibration rainbow band, a thin stand,
//! and a small "✓" check-mark overlay in the corner.
//!
//! Run: `cargo run --example generate_icon -p lg-cli`
//! Output: `crates/lg-cli/assets/app.ico`

use std::path::Path;

fn main() {
    let out = Path::new("crates/lg-cli/assets/app.ico");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }

    let sizes: &[u32] = &[16, 32, 48, 256];
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| render_icon(s)).collect();

    let ico = build_ico(&images, sizes);
    std::fs::write(out, &ico).unwrap();
    println!("✓ Icon written to {}", out.display());
}

// ─── ICO file format ─────────────────────────────────────────────

fn build_ico(images: &[Vec<u8>], sizes: &[u32]) -> Vec<u8> {
    let count = images.len() as u16;

    // ICO header: 6 bytes
    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes()); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    ico.extend_from_slice(&count.to_le_bytes()); // image count

    // Each directory entry: 16 bytes
    // Data starts after header (6) + entries (16 * count)
    let header_size = 6 + 16 * count as u32;
    let mut offset = header_size;

    for (i, img) in images.iter().enumerate() {
        let s = sizes[i];
        let png_data = encode_png(img, s);
        let w = if s >= 256 { 0u8 } else { s as u8 };
        let h = w;
        ico.push(w); // width
        ico.push(h); // height
        ico.push(0); // color palette
        ico.push(0); // reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // color planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        ico.extend_from_slice(&(png_data.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += png_data.len() as u32;
    }

    // Append PNG data for each image
    for (i, img) in images.iter().enumerate() {
        let png_data = encode_png(img, sizes[i]);
        ico.extend_from_slice(&png_data);
    }

    ico
}

// ─── Minimal PNG encoder (no dependencies) ───────────────────────

fn encode_png(rgba: &[u8], size: u32) -> Vec<u8> {
    let mut png = Vec::new();
    // PNG signature
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // IHDR chunk
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&size.to_be_bytes()); // width
    ihdr.extend_from_slice(&size.to_be_bytes()); // height
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type: RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_chunk(&mut png, b"IHDR", &ihdr);

    // IDAT chunk — raw pixel rows with filter byte 0 (None)
    let mut raw = Vec::new();
    for y in 0..size {
        raw.push(0); // filter: None
        let row_start = (y * size * 4) as usize;
        let row_end = row_start + (size * 4) as usize;
        raw.extend_from_slice(&rgba[row_start..row_end]);
    }
    let compressed = deflate_compress(&raw);
    write_chunk(&mut png, b"IDAT", &compressed);

    // IEND chunk
    write_chunk(&mut png, b"IEND", &[]);

    png
}

fn write_chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(data);
    let mut crc_data = Vec::with_capacity(4 + data.len());
    crc_data.extend_from_slice(tag);
    crc_data.extend_from_slice(data);
    let crc = crc32(&crc_data);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Minimal DEFLATE compression using zlib-wrapped uncompressed blocks.
/// Not optimally compressed but fully valid — keeps the generator dependency-free.
fn deflate_compress(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    // zlib header: CM=8 (deflate), CINFO=7 (32K window), FCHECK adjusted
    out.push(0x78);
    out.push(0x01);

    // Split into uncompressed DEFLATE blocks (max 65535 bytes each)
    let chunks: Vec<&[u8]> = data.chunks(65535).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let is_last = i == chunks.len() - 1;
        out.push(if is_last { 0x01 } else { 0x00 }); // BFINAL + BTYPE=00
        let len = chunk.len() as u16;
        let nlen = !len;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(chunk);
    }

    // Adler-32 checksum
    let adler = adler32(data);
    out.extend_from_slice(&adler.to_be_bytes());

    out
}

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

// ─── Icon rendering ──────────────────────────────────────────────

/// Render the icon at the given pixel size. Returns RGBA pixel data.
fn render_icon(size: u32) -> Vec<u8> {
    let s = size as f64;
    let mut pixels = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let fx = x as f64 / s;
            let fy = y as f64 / s;
            let color = pixel_at(fx, fy, s);
            let idx = ((y * size + x) * 4) as usize;
            pixels[idx] = color.0; // R
            pixels[idx + 1] = color.1; // G
            pixels[idx + 2] = color.2; // B
            pixels[idx + 3] = color.3; // A
        }
    }

    pixels
}

/// Determine the RGBA color for a normalized coordinate (0..1, 0..1).
/// Draws a stylised LCD monitor with color calibration gradient.
fn pixel_at(fx: f64, fy: f64, size: f64) -> (u8, u8, u8, u8) {
    // Anti-aliasing helper: pixels at boundary get partial alpha
    let aa = 1.0 / size;

    // ── Monitor bezel ────────────────────────────────────────
    // Outer bezel: rounded rectangle from ~0.08 to ~0.92 horizontally,
    // ~0.05 to ~0.72 vertically
    let bezel_l = 0.06;
    let bezel_r = 0.94;
    let bezel_t = 0.04;
    let bezel_b = 0.68;
    let bezel_r_rad = 0.06; // corner radius (normalized)

    let in_bezel = rounded_rect(fx, fy, bezel_l, bezel_t, bezel_r, bezel_b, bezel_r_rad, aa);

    // ── Screen area (inside bezel) ───────────────────────────
    let screen_l = 0.12;
    let screen_r = 0.88;
    let screen_t = 0.09;
    let screen_b = 0.63;
    let screen_r_rad = 0.03;

    let in_screen = rounded_rect(
        fx,
        fy,
        screen_l,
        screen_t,
        screen_r,
        screen_b,
        screen_r_rad,
        aa,
    );

    // ── Stand neck ───────────────────────────────────────────
    let neck_l = 0.42;
    let neck_r = 0.58;
    let neck_t = 0.68;
    let neck_b = 0.82;
    let in_neck = rect_aa(fx, fy, neck_l, neck_t, neck_r, neck_b, aa);

    // ── Stand base ───────────────────────────────────────────
    let base_l = 0.25;
    let base_r = 0.75;
    let base_t = 0.82;
    let base_b = 0.90;
    let base_rad = 0.03;
    let in_base = rounded_rect(fx, fy, base_l, base_t, base_r, base_b, base_rad, aa);

    // ── Color calibration gradient on screen ─────────────────
    // A horizontal rainbow band in the middle third of the screen
    let band_t = 0.30;
    let band_b = 0.48;
    let in_band = in_screen.min(rect_aa(fx, fy, screen_l, band_t, screen_r, band_b, aa));

    // ── Checkmark in bottom-right of screen ──────────────────
    let check_cx = 0.78;
    let check_cy = 0.55;
    let check_r = 0.06;
    let in_check_circle = circle_aa(fx, fy, check_cx, check_cy, check_r, aa);
    let in_check_mark = checkmark_aa(fx, fy, check_cx, check_cy, check_r * 0.6, aa);

    // ── Compose layers ───────────────────────────────────────

    // Start transparent
    let mut r = 0.0f64;
    let mut g = 0.0f64;
    let mut b = 0.0f64;
    let mut a = 0.0f64;

    // Layer 1: Bezel (dark charcoal #2D2D2D)
    let bezel_color = (0.176, 0.176, 0.176);
    blend(&mut r, &mut g, &mut b, &mut a, bezel_color, in_bezel);

    // Layer 1b: Stand neck (slightly lighter #3A3A3A)
    let neck_color = (0.227, 0.227, 0.227);
    blend(&mut r, &mut g, &mut b, &mut a, neck_color, in_neck);

    // Layer 1c: Stand base (same as bezel)
    blend(&mut r, &mut g, &mut b, &mut a, bezel_color, in_base);

    // Layer 2: Screen background (dark blue-black #0A0E1A)
    let screen_bg = (0.039, 0.055, 0.102);
    blend(&mut r, &mut g, &mut b, &mut a, screen_bg, in_screen);

    // Layer 3: Rainbow calibration band
    if in_band > 0.001 {
        let t = (fx - screen_l) / (screen_r - screen_l); // 0..1 across screen
        let (cr, cg, cb) = rainbow_gradient(t);
        // Slight vertical fade
        let band_fy = (fy - band_t) / (band_b - band_t);
        let intensity = 1.0 - (band_fy - 0.5).abs() * 1.2;
        let intensity = intensity.clamp(0.3, 1.0);
        blend(
            &mut r,
            &mut g,
            &mut b,
            &mut a,
            (cr * intensity, cg * intensity, cb * intensity),
            in_band,
        );
    }

    // Layer 4: Green check circle (#22C55E)
    let check_bg = (0.133, 0.773, 0.369);
    blend(&mut r, &mut g, &mut b, &mut a, check_bg, in_check_circle);

    // Layer 5: White checkmark on the green circle
    let check_fg = (1.0, 1.0, 1.0);
    blend(&mut r, &mut g, &mut b, &mut a, check_fg, in_check_mark);

    // Convert to u8
    (
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
        (a * 255.0).round() as u8,
    )
}

// ─── Primitive shapes with anti-aliasing ─────────────────────────

/// Signed distance for a rounded rectangle, returns 0.0..1.0 coverage.
#[allow(clippy::too_many_arguments)]
fn rounded_rect(fx: f64, fy: f64, l: f64, t: f64, r: f64, b: f64, rad: f64, aa: f64) -> f64 {
    let cx = (l + r) / 2.0;
    let cy = (t + b) / 2.0;
    let hw = (r - l) / 2.0;
    let hh = (b - t) / 2.0;

    let dx = (fx - cx).abs() - (hw - rad);
    let dy = (fy - cy).abs() - (hh - rad);

    let outside_corner = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() - rad;
    let inside = dx.max(dy).min(0.0);
    let dist = outside_corner + inside;

    smoothstep(aa, -aa, dist)
}

fn rect_aa(fx: f64, fy: f64, l: f64, t: f64, r: f64, b: f64, aa: f64) -> f64 {
    let dx = ((fx - l).min(r - fx)).min(1.0);
    let dy = ((fy - t).min(b - fy)).min(1.0);
    let d = dx.min(dy);
    smoothstep(-aa, aa, d)
}

fn circle_aa(fx: f64, fy: f64, cx: f64, cy: f64, radius: f64, aa: f64) -> f64 {
    let dx = fx - cx;
    let dy = fy - cy;
    let dist = (dx * dx + dy * dy).sqrt() - radius;
    smoothstep(aa, -aa, dist)
}

/// A checkmark shape (two line segments: short descending + long ascending).
fn checkmark_aa(fx: f64, fy: f64, cx: f64, cy: f64, scale: f64, aa: f64) -> f64 {
    // Checkmark points (normalized around 0,0):
    // left arm start: (-0.6, 0.0), vertex: (-0.1, 0.5), right arm end: (0.7, -0.5)
    let lx = (fx - cx) / scale;
    let ly = (fy - cy) / scale;

    let d1 = dist_to_segment(lx, ly, -0.55, -0.05, -0.1, 0.4);
    let d2 = dist_to_segment(lx, ly, -0.1, 0.4, 0.6, -0.45);

    let thickness = 0.28;
    let d = d1.min(d2) - thickness;
    smoothstep(aa / scale, -aa / scale, d)
}

fn dist_to_segment(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let t = ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy);
    let t = t.clamp(0.0, 1.0);
    let proj_x = ax + t * dx;
    let proj_y = ay + t * dy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Alpha-premultiplied blend: layer `color` with coverage `alpha` on top.
fn blend(r: &mut f64, g: &mut f64, b: &mut f64, a: &mut f64, color: (f64, f64, f64), alpha: f64) {
    *r = *r * (1.0 - alpha) + color.0 * alpha;
    *g = *g * (1.0 - alpha) + color.1 * alpha;
    *b = *b * (1.0 - alpha) + color.2 * alpha;
    *a = *a * (1.0 - alpha) + alpha;
}

/// HSL-like rainbow gradient: red → orange → yellow → green → cyan → blue → violet
fn rainbow_gradient(t: f64) -> (f64, f64, f64) {
    // Six-segment HSV hue sweep with boosted saturation
    let h = t * 300.0; // 0° (red) to 300° (magenta)
    let s = 0.85;
    let v = 0.95;
    hsv_to_rgb(h, s, v)
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r + m, g + m, b + m)
}
