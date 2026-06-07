use crate::models::AppSettings;
use crate::services::image_loader;
use crate::services::image_processor::PostImageSize;

use anyhow::{Context, Result};
use image::imageops;
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatermarkConfig {
    pub enabled: bool,
    pub image_ref: String,
    pub opacity_percent: u32,
    pub scale_percent: u32,
    pub size_mode: WatermarkSizeMode,
    pub width_px: u32,
    pub height_px: u32,
    pub position_mode: WatermarkPositionMode,
    pub preset: WatermarkPreset,
    pub margin_x: u32,
    pub margin_y: u32,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatermarkSizeMode {
    Scale,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatermarkPositionMode {
    Preset,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatermarkPreset {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl WatermarkConfig {
    pub fn from_settings(settings: &AppSettings) -> Self {
        Self {
            enabled: settings.watermark_enabled,
            image_ref: settings.watermark_image.clone(),
            opacity_percent: settings.watermark_opacity.clamp(0, 100),
            scale_percent: settings.watermark_scale_percent.clamp(5, 80),
            size_mode: WatermarkSizeMode::from_str(&settings.watermark_size_mode),
            width_px: settings.watermark_width_px,
            height_px: settings.watermark_height_px,
            position_mode: WatermarkPositionMode::from_str(&settings.watermark_position_mode),
            preset: WatermarkPreset::from_str(&settings.watermark_preset),
            margin_x: settings.watermark_margin_x,
            margin_y: settings.watermark_margin_y,
            x: settings.watermark_x,
            y: settings.watermark_y,
        }
    }

    pub fn is_active(&self) -> bool {
        self.enabled && !self.image_ref.trim().is_empty()
    }
}

impl WatermarkSizeMode {
    pub fn from_str(value: &str) -> Self {
        if value == "custom" {
            Self::Custom
        } else {
            Self::Scale
        }
    }
}

impl WatermarkPositionMode {
    pub fn from_str(value: &str) -> Self {
        if value == "manual" {
            Self::Manual
        } else {
            Self::Preset
        }
    }
}

impl WatermarkPreset {
    pub fn from_str(value: &str) -> Self {
        match value {
            "top_left" => Self::TopLeft,
            "top_center" => Self::TopCenter,
            "top_right" => Self::TopRight,
            "center_left" => Self::CenterLeft,
            "center" => Self::Center,
            "center_right" => Self::CenterRight,
            "bottom_left" => Self::BottomLeft,
            "bottom_center" => Self::BottomCenter,
            _ => Self::BottomRight,
        }
    }
}

pub fn watermark_dimensions(
    canvas: PostImageSize,
    config: &WatermarkConfig,
    natural_w: u32,
    natural_h: u32,
) -> (u32, u32) {
    if config.size_mode == WatermarkSizeMode::Custom
        && config.width_px > 0
        && config.height_px > 0
    {
        return (
            config.width_px.min(canvas.width).max(1),
            config.height_px.min(canvas.height).max(1),
        );
    }

    let target_w = ((canvas.width as f32 * config.scale_percent as f32) / 100.0)
        .round()
        .max(1.0) as u32;
    let aspect = natural_h as f32 / natural_w.max(1) as f32;
    let target_h = (target_w as f32 * aspect).round().max(1.0) as u32;
    (
        target_w.min(canvas.width).max(1),
        target_h.min(canvas.height).max(1),
    )
}

pub fn compute_watermark_position(
    canvas: PostImageSize,
    wm_w: u32,
    wm_h: u32,
    config: &WatermarkConfig,
) -> (u32, u32) {
    let max_x = canvas.width.saturating_sub(wm_w);
    let max_y = canvas.height.saturating_sub(wm_h);

    if config.position_mode == WatermarkPositionMode::Manual {
        return (config.x.min(max_x), config.y.min(max_y));
    }

    let mx = config.margin_x.min(max_x);
    let my = config.margin_y.min(max_y);

    match config.preset {
        WatermarkPreset::TopLeft => (mx, my),
        WatermarkPreset::TopCenter => (max_x / 2, my),
        WatermarkPreset::TopRight => (max_x.saturating_sub(mx), my),
        WatermarkPreset::CenterLeft => (mx, max_y / 2),
        WatermarkPreset::Center => (max_x / 2, max_y / 2),
        WatermarkPreset::CenterRight => (max_x.saturating_sub(mx), max_y / 2),
        WatermarkPreset::BottomLeft => (mx, max_y.saturating_sub(my)),
        WatermarkPreset::BottomCenter => (max_x / 2, max_y.saturating_sub(my)),
        WatermarkPreset::BottomRight => (max_x.saturating_sub(mx), max_y.saturating_sub(my)),
    }
}

pub fn load_watermark_natural_size(path: &Path) -> Result<(u32, u32)> {
    if is_svg_path(path) {
        let bytes = std::fs::read(path).with_context(|| format!("Read {}", path.display()))?;
        let tree = usvg::Tree::from_data(&bytes, &usvg::Options::default())
            .context("Parse SVG watermark")?;
        let size = tree.size();
        let w = size.width().max(1.0).round() as u32;
        let h = size.height().max(1.0).round() as u32;
        Ok((w.max(1), h.max(1)))
    } else {
        let img = image::open(path).with_context(|| format!("Open {}", path.display()))?;
        let (w, h) = img.dimensions();
        Ok((w.max(1), h.max(1)))
    }
}

pub fn load_watermark_raster(path: &Path, width: u32, height: u32) -> Result<DynamicImage> {
    let width = width.max(1);
    let height = height.max(1);
    if is_svg_path(path) {
        rasterize_svg(path, width, height)
    } else {
        let img = image::open(path).with_context(|| format!("Open {}", path.display()))?;
        Ok(img.resize_exact(
            width,
            height,
            image::imageops::FilterType::Triangle,
        ))
    }
}

pub fn apply_watermark(
    base: &DynamicImage,
    watermark: &DynamicImage,
    canvas: PostImageSize,
    config: &WatermarkConfig,
    natural_w: u32,
    natural_h: u32,
) -> DynamicImage {
    let (wm_w, wm_h) = watermark_dimensions(canvas, config, natural_w, natural_h);
    let resized = if watermark.width() == wm_w && watermark.height() == wm_h {
        watermark.clone()
    } else {
        watermark.resize_exact(wm_w, wm_h, image::imageops::FilterType::Triangle)
    };

    let opacity = config.opacity_percent as f32 / 100.0;
    let mut wm_rgba = resized.to_rgba8();
    apply_opacity(&mut wm_rgba, opacity);

    let (x, y) = compute_watermark_position(canvas, wm_w, wm_h, config);
    let mut base_rgba = base.to_rgba8();
    imageops::overlay(&mut base_rgba, &wm_rgba, x as i64, y as i64);
    DynamicImage::ImageRgba8(base_rgba)
}

pub fn apply_watermark_from_settings(
    base: &DynamicImage,
    data_dir: &Path,
    canvas: PostImageSize,
    config: &WatermarkConfig,
) -> Result<DynamicImage> {
    if !config.is_active() {
        return Ok(base.clone());
    }

    let path = image_loader::resolve_local_image_path(data_dir, &config.image_ref)
        .with_context(|| format!("Watermark file {}", config.image_ref))?;
    let (natural_w, natural_h) = load_watermark_natural_size(&path)?;
    let (wm_w, wm_h) = watermark_dimensions(canvas, config, natural_w, natural_h);
    let watermark = load_watermark_raster(&path, wm_w, wm_h)?;
    Ok(apply_watermark(
        base,
        &watermark,
        canvas,
        config,
        natural_w,
        natural_h,
    ))
}

fn is_svg_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("svg"))
        .unwrap_or(false)
}

fn rasterize_svg(path: &Path, width: u32, height: u32) -> Result<DynamicImage> {
    let bytes = std::fs::read(path).with_context(|| format!("Read {}", path.display()))?;
    let tree = usvg::Tree::from_data(&bytes, &usvg::Options::default())
        .context("Parse SVG watermark")?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).context("Allocate SVG pixmap")?;
    pixmap.fill(tiny_skia::Color::TRANSPARENT);

    let scale_x = width as f32 / tree.size().width().max(1.0);
    let scale_y = height as f32 / tree.size().height().max(1.0);
    let transform = tiny_skia::Transform::from_scale(scale_x, scale_y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Ok(DynamicImage::ImageRgba8(pixmap_to_rgba(&pixmap)?))
}

fn pixmap_to_rgba(pixmap: &tiny_skia::Pixmap) -> Result<RgbaImage> {
    let width = pixmap.width();
    let height = pixmap.height();
    let mut img = RgbaImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let pixel = pixmap.pixel(x, y).context("SVG pixel")?;
            let a = pixel.alpha() as f32 / 255.0;
            if a <= 0.0 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                continue;
            }
            let r = (pixel.red() as f32 / a).round().clamp(0.0, 255.0) as u8;
            let g = (pixel.green() as f32 / a).round().clamp(0.0, 255.0) as u8;
            let b = (pixel.blue() as f32 / a).round().clamp(0.0, 255.0) as u8;
            img.put_pixel(x, y, Rgba([r, g, b, pixel.alpha()]));
        }
    }

    Ok(img)
}

fn apply_opacity(img: &mut RgbaImage, opacity: f32) {
    if opacity >= 1.0 {
        return;
    }
    if opacity <= 0.0 {
        for pixel in img.pixels_mut() {
            pixel.0[3] = 0;
        }
        return;
    }
    for pixel in img.pixels_mut() {
        pixel.0[3] = (pixel.0[3] as f32 * opacity).round().clamp(0.0, 255.0) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    fn solid_wm(w: u32, h: u32) -> DynamicImage {
        let mut img: RgbaImage = ImageBuffer::new(w, h);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([255, 0, 0, 200]);
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn preset_bottom_right_uses_margin() {
        let canvas = PostImageSize {
            width: 1280,
            height: 720,
        };
        let config = WatermarkConfig {
            enabled: true,
            image_ref: "local:watermark/test.png".to_string(),
            opacity_percent: 100,
            scale_percent: 10,
            size_mode: WatermarkSizeMode::Scale,
            width_px: 0,
            height_px: 0,
            position_mode: WatermarkPositionMode::Preset,
            preset: WatermarkPreset::BottomRight,
            margin_x: 30,
            margin_y: 40,
            x: 0,
            y: 0,
        };
        let (wm_w, wm_h) = watermark_dimensions(canvas, &config, 100, 50);
        let (x, y) = compute_watermark_position(canvas, wm_w, wm_h, &config);
        assert_eq!(x, canvas.width - wm_w - 30);
        assert_eq!(y, canvas.height - wm_h - 40);
    }

    #[test]
    fn custom_size_is_used() {
        let canvas = PostImageSize {
            width: 1280,
            height: 720,
        };
        let config = WatermarkConfig {
            enabled: true,
            image_ref: String::new(),
            opacity_percent: 100,
            scale_percent: 10,
            size_mode: WatermarkSizeMode::Custom,
            width_px: 240,
            height_px: 120,
            position_mode: WatermarkPositionMode::Manual,
            preset: WatermarkPreset::Center,
            margin_x: 0,
            margin_y: 0,
            x: 10,
            y: 20,
        };
        let (wm_w, wm_h) = watermark_dimensions(canvas, &config, 100, 50);
        assert_eq!(wm_w, 240);
        assert_eq!(wm_h, 120);
    }
}
