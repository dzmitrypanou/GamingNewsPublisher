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
    pub backdrop: WatermarkBackdrop,
    pub backdrop_opacity_percent: u32,
    pub backdrop_padding: u32,
    pub backdrop_color: String,
    pub backdrop_logo_offset_x: u32,
    pub backdrop_logo_offset_y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatermarkBackdrop {
    None,
    DarkRect,
    DarkPill,
    Shadow,
    DarkGlow,
    BottomBar,
    TopBar,
    BottomGradient,
    TopGradient,
    LeftStrip,
    RightStrip,
    Vignette,
    CornerFade,
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
            backdrop: WatermarkBackdrop::from_str(&settings.watermark_backdrop),
            backdrop_opacity_percent: settings.watermark_backdrop_opacity.clamp(0, 100),
            backdrop_padding: settings.watermark_backdrop_padding.clamp(0, 80),
            backdrop_color: settings.watermark_backdrop_color.clone(),
            backdrop_logo_offset_x: normalize_backdrop_logo_offset(
                settings.watermark_backdrop_logo_x,
                settings.watermark_backdrop_padding,
            ),
            backdrop_logo_offset_y: normalize_backdrop_logo_offset(
                settings.watermark_backdrop_logo_y,
                settings.watermark_backdrop_padding,
            ),
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

    fn corner(self) -> Corner {
        match self {
            Self::TopLeft => Corner::TopLeft,
            Self::TopRight => Corner::TopRight,
            Self::BottomLeft => Corner::BottomLeft,
            Self::TopCenter => Corner::TopLeft,
            Self::BottomCenter => Corner::BottomRight,
            Self::CenterLeft => Corner::BottomLeft,
            Self::CenterRight => Corner::TopRight,
            Self::Center => Corner::BottomRight,
            _ => Corner::BottomRight,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl WatermarkBackdrop {
    pub fn from_str(value: &str) -> Self {
        match value {
            "dark_rect" | "light_rect" => Self::DarkRect,
            "dark_pill" | "light_pill" => Self::DarkPill,
            "shadow" => Self::Shadow,
            "dark_outline" | "light_outline" => Self::None,
            "dark_glow" => Self::DarkGlow,
            "bottom_bar" => Self::BottomBar,
            "top_bar" => Self::TopBar,
            "bottom_gradient" => Self::BottomGradient,
            "top_gradient" => Self::TopGradient,
            "left_strip" => Self::LeftStrip,
            "right_strip" => Self::RightStrip,
            "vignette" => Self::Vignette,
            "corner_fade" => Self::CornerFade,
            _ => Self::None,
        }
    }

    pub fn tied_to_watermark(self) -> bool {
        !matches!(
            self,
            Self::None
                | Self::BottomBar
                | Self::TopBar
                | Self::BottomGradient
                | Self::TopGradient
                | Self::LeftStrip
                | Self::RightStrip
                | Self::Vignette
                | Self::CornerFade
        )
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
    compute_position_for_box(canvas, wm_w, wm_h, config)
}

fn compute_position_for_box(
    canvas: PostImageSize,
    box_w: u32,
    box_h: u32,
    config: &WatermarkConfig,
) -> (u32, u32) {
    let max_x = canvas.width.saturating_sub(box_w);
    let max_y = canvas.height.saturating_sub(box_h);

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

#[derive(Debug, Clone, Copy)]
struct WatermarkLayout {
    logo_x: u32,
    logo_y: u32,
    backdrop_x: i32,
    backdrop_y: i32,
    backdrop_w: i32,
    backdrop_h: i32,
}

fn normalize_backdrop_logo_offset(raw: u32, padding: u32) -> u32 {
    let slack = padding.saturating_mul(2);
    if slack == 0 {
        return 0;
    }
    let value = if raw <= 100 && raw % 50 == 0 {
        ((slack as u64 * raw as u64) / 100) as u32
    } else {
        raw
    };
    value.min(slack)
}

fn compute_watermark_layout(
    canvas: PostImageSize,
    wm_w: u32,
    wm_h: u32,
    config: &WatermarkConfig,
) -> WatermarkLayout {
    if !config.backdrop.tied_to_watermark() {
        let (logo_x, logo_y) = compute_watermark_position(canvas, wm_w, wm_h, config);
        return WatermarkLayout {
            logo_x,
            logo_y,
            backdrop_x: 0,
            backdrop_y: 0,
            backdrop_w: 0,
            backdrop_h: 0,
        };
    }

    let pad = config.backdrop_padding;
    let backdrop_w = wm_w.saturating_add(pad.saturating_mul(4));
    let backdrop_h = wm_h.saturating_add(pad.saturating_mul(4));
    let slack_x = pad.saturating_mul(2);
    let slack_y = pad.saturating_mul(2);
    let logo_x_offset = config
        .backdrop_logo_offset_x
        .min(slack_x);
    let logo_y_offset = config
        .backdrop_logo_offset_y
        .min(slack_y);

    let (mut backdrop_x, mut backdrop_y) = if config.position_mode == WatermarkPositionMode::Manual {
        let max_logo_x = canvas.width.saturating_sub(wm_w);
        let max_logo_y = canvas.height.saturating_sub(wm_h);
        let logo_x = config.x.min(max_logo_x);
        let logo_y = config.y.min(max_logo_y);
        (
            logo_x.saturating_sub(pad).saturating_sub(logo_x_offset) as i32,
            logo_y.saturating_sub(pad).saturating_sub(logo_y_offset) as i32,
        )
    } else {
        let (bx, by) = compute_position_for_box(canvas, backdrop_w, backdrop_h, config);
        (bx as i32, by as i32)
    };

    let max_backdrop_x = canvas.width.saturating_sub(backdrop_w) as i32;
    let max_backdrop_y = canvas.height.saturating_sub(backdrop_h) as i32;
    backdrop_x = backdrop_x.clamp(0, max_backdrop_x);
    backdrop_y = backdrop_y.clamp(0, max_backdrop_y);

    let logo_x = backdrop_x as u32 + pad + logo_x_offset;
    let logo_y = backdrop_y as u32 + pad + logo_y_offset;

    WatermarkLayout {
        logo_x,
        logo_y,
        backdrop_x,
        backdrop_y,
        backdrop_w: backdrop_w as i32,
        backdrop_h: backdrop_h as i32,
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

    let layout = compute_watermark_layout(canvas, wm_w, wm_h, config);
    let mut base_rgba = base.to_rgba8();
    draw_backdrop(&mut base_rgba, canvas, config, &layout, wm_w, wm_h);
    imageops::overlay(
        &mut base_rgba,
        &wm_rgba,
        layout.logo_x as i64,
        layout.logo_y as i64,
    );
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

fn backdrop_strength(config: &WatermarkConfig) -> f32 {
    config.backdrop_opacity_percent as f32 / 100.0
}

fn rgba_alpha(r: u8, g: u8, b: u8, alpha: f32) -> Rgba<u8> {
    Rgba([
        r,
        g,
        b,
        (alpha * 255.0).round().clamp(0.0, 255.0) as u8,
    ])
}

fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, fg: Rgba<u8>) {
    if x >= img.width() || y >= img.height() || fg[3] == 0 {
        return;
    }
    let bg = *img.get_pixel(x, y);
    let a = fg[3] as f32 / 255.0;
    let inv = 1.0 - a;
    let blended = Rgba([
        (fg[0] as f32 * a + bg[0] as f32 * inv).round() as u8,
        (fg[1] as f32 * a + bg[1] as f32 * inv).round() as u8,
        (fg[2] as f32 * a + bg[2] as f32 * inv).round() as u8,
        (255.0 * (a + inv * bg[3] as f32 / 255.0)).round().clamp(0.0, 255.0) as u8,
    ]);
    img.put_pixel(x, y, blended);
}

fn fill_rect(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>) {
    let w = img.width() as i32;
    let h = img.height() as i32;
    for y in y0.max(0)..y1.min(h) {
        for x in x0.max(0)..x1.min(w) {
            blend_pixel(img, x as u32, y as u32, color);
        }
    }
}

fn inside_round_rect(px: i32, py: i32, rx: i32, ry: i32, rw: i32, rh: i32, radius: i32) -> bool {
    if px < rx || py < ry || px >= rx + rw || py >= ry + rh {
        return false;
    }
    let r = radius.max(1);
    let right = rx + rw;
    let bottom = ry + rh;

    if px < rx + r && py < ry + r {
        let dx = px - (rx + r);
        let dy = py - (ry + r);
        return dx * dx + dy * dy <= r * r;
    }
    if px >= right - r && py < ry + r {
        let dx = px - (right - r);
        let dy = py - (ry + r);
        return dx * dx + dy * dy <= r * r;
    }
    if px < rx + r && py >= bottom - r {
        let dx = px - (rx + r);
        let dy = py - (bottom - r);
        return dx * dx + dy * dy <= r * r;
    }
    if px >= right - r && py >= bottom - r {
        let dx = px - (right - r);
        let dy = py - (bottom - r);
        return dx * dx + dy * dy <= r * r;
    }
    true
}

fn fill_round_rect(
    img: &mut RgbaImage,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: i32,
    color: Rgba<u8>,
) {
    for py in y..y + h {
        for px in x..x + w {
            if inside_round_rect(px, py, x, y, w, h, radius) {
                blend_pixel(img, px as u32, py as u32, color);
            }
        }
    }
}

fn parse_backdrop_color(hex: &str) -> (u8, u8, u8) {
    let raw = hex.trim().trim_start_matches('#');
    if raw.len() >= 6 {
        let r = u8::from_str_radix(&raw[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&raw[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&raw[4..6], 16).unwrap_or(0);
        return (r, g, b);
    }
    (0, 0, 0)
}

fn nearest_corner(
    canvas: PostImageSize,
    x: u32,
    y: u32,
    wm_w: u32,
    wm_h: u32,
    config: &WatermarkConfig,
) -> Corner {
    if config.position_mode == WatermarkPositionMode::Preset {
        return config.preset.corner();
    }
    let cx = x as f32 + wm_w as f32 / 2.0;
    let cy = y as f32 + wm_h as f32 / 2.0;
    let left = cx < canvas.width as f32 / 2.0;
    let top = cy < canvas.height as f32 / 2.0;
    match (left, top) {
        (true, true) => Corner::TopLeft,
        (false, true) => Corner::TopRight,
        (true, false) => Corner::BottomLeft,
        (false, false) => Corner::BottomRight,
    }
}

fn draw_backdrop(
    img: &mut RgbaImage,
    canvas: PostImageSize,
    config: &WatermarkConfig,
    layout: &WatermarkLayout,
    wm_w: u32,
    wm_h: u32,
) {
    if config.backdrop == WatermarkBackdrop::None {
        return;
    }

    let strength = backdrop_strength(config);
    if strength <= 0.0 {
        return;
    }

    let (r, g, b) = parse_backdrop_color(&config.backdrop_color);
    let fill = rgba_alpha(r, g, b, 0.72 * strength);
    let shadow = rgba_alpha(r, g, b, 0.45 * strength);

    let (bx, by, bw, bh) = if config.backdrop.tied_to_watermark() {
        (
            layout.backdrop_x,
            layout.backdrop_y,
            layout.backdrop_w,
            layout.backdrop_h,
        )
    } else {
        let pad = config.backdrop_padding as i32;
        let x0 = layout.logo_x as i32 - pad;
        let y0 = layout.logo_y as i32 - pad;
        let w = wm_w as i32 + pad * 2;
        let h = wm_h as i32 + pad * 2;
        let max_x = canvas.width as i32;
        let max_y = canvas.height as i32;
        let cx0 = x0.max(0);
        let cy0 = y0.max(0);
        let cx1 = (x0 + w).min(max_x);
        let cy1 = (y0 + h).min(max_y);
        (cx0, cy0, (cx1 - cx0).max(1), (cy1 - cy0).max(1))
    };
    let radius = ((config.backdrop_padding as i32).max(8)).min(bw / 2).min(bh / 2);

    match config.backdrop {
        WatermarkBackdrop::None => {}
        WatermarkBackdrop::DarkRect => fill_rect(img, bx, by, bx + bw, by + bh, fill),
        WatermarkBackdrop::DarkPill => fill_round_rect(img, bx, by, bw, bh, radius, fill),
        WatermarkBackdrop::Shadow => {
            fill_round_rect(img, bx + 3, by + 4, bw, bh, radius, shadow);
            fill_round_rect(img, bx + 1, by + 2, bw, bh, radius, shadow);
        }
        WatermarkBackdrop::DarkGlow => {
            for expand in (1..=4).rev() {
                let alpha = 0.18 * strength * (expand as f32 / 4.0);
                fill_round_rect(
                    img,
                    bx - expand,
                    by - expand,
                    bw + expand * 2,
                    bh + expand * 2,
                    radius + expand,
                    rgba_alpha(r, g, b, alpha),
                );
            }
            fill_round_rect(img, bx, by, bw, bh, radius, rgba_alpha(r, g, b, 0.55 * strength));
        }
        WatermarkBackdrop::BottomBar => {
            let bar_h = (canvas.height as f32 * 0.16).round().max(48.0) as i32;
            fill_rect(
                img,
                0,
                canvas.height as i32 - bar_h,
                canvas.width as i32,
                canvas.height as i32,
                fill,
            );
        }
        WatermarkBackdrop::TopBar => {
            let bar_h = (canvas.height as f32 * 0.16).round().max(48.0) as i32;
            fill_rect(img, 0, 0, canvas.width as i32, bar_h, fill);
        }
        WatermarkBackdrop::BottomGradient => draw_vertical_gradient(
            img,
            0,
            (canvas.height as f32 * 0.55) as i32,
            canvas.width as i32,
            canvas.height as i32,
            r,
            g,
            b,
            0.0,
            strength * 0.85,
        ),
        WatermarkBackdrop::TopGradient => draw_vertical_gradient(
            img,
            0,
            0,
            canvas.width as i32,
            (canvas.height as f32 * 0.45) as i32,
            r,
            g,
            b,
            strength * 0.85,
            0.0,
        ),
        WatermarkBackdrop::LeftStrip => {
            let strip_w = (canvas.width as f32 * 0.22).round().max(80.0) as i32;
            fill_rect(img, 0, 0, strip_w, canvas.height as i32, fill);
        }
        WatermarkBackdrop::RightStrip => {
            let strip_w = (canvas.width as f32 * 0.22).round().max(80.0) as i32;
            fill_rect(
                img,
                canvas.width as i32 - strip_w,
                0,
                canvas.width as i32,
                canvas.height as i32,
                fill,
            );
        }
        WatermarkBackdrop::Vignette => draw_vignette(img, r, g, b, strength * 0.9),
        WatermarkBackdrop::CornerFade => {
            draw_corner_fade(
                img,
                canvas,
                nearest_corner(canvas, layout.logo_x, layout.logo_y, wm_w, wm_h, config),
                r,
                g,
                b,
                strength,
            );
        }
    }
}

fn draw_vertical_gradient(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    r: u8,
    g: u8,
    b: u8,
    top_alpha: f32,
    bottom_alpha: f32,
) {
    let h = (y1 - y0).max(1);
    for y in y0..y1 {
        let t = (y - y0) as f32 / h as f32;
        let alpha = top_alpha + (bottom_alpha - top_alpha) * t;
        let color = rgba_alpha(r, g, b, alpha);
        fill_rect(img, x0, y, x1, y + 1, color);
    }
}

fn draw_vignette(img: &mut RgbaImage, r: u8, g: u8, b: u8, strength: f32) {
    let w = img.width();
    let h = img.height();
    for y in 0..h {
        for x in 0..w {
            let nx = x as f32 / w as f32;
            let ny = y as f32 / h as f32;
            let dx = (nx - 0.5).abs() * 2.0;
            let dy = (ny - 0.5).abs() * 2.0;
            let edge = dx.max(dy);
            let alpha = ((edge - 0.35).max(0.0) / 0.65).powf(1.4) * strength;
            if alpha > 0.01 {
                blend_pixel(img, x, y, rgba_alpha(r, g, b, alpha));
            }
        }
    }
}

fn draw_corner_fade(
    img: &mut RgbaImage,
    canvas: PostImageSize,
    corner: Corner,
    r: u8,
    g: u8,
    b: u8,
    strength: f32,
) {
    let w = canvas.width;
    let h = canvas.height;
    let reach_x = (w as f32 * 0.55) as u32;
    let reach_y = (h as f32 * 0.55) as u32;
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = match corner {
                Corner::TopLeft => (x, y),
                Corner::TopRight => (w - 1 - x, y),
                Corner::BottomLeft => (x, h - 1 - y),
                Corner::BottomRight => (w - 1 - x, h - 1 - y),
            };
            if dx >= reach_x || dy >= reach_y {
                continue;
            }
            let tx = dx as f32 / reach_x as f32;
            let ty = dy as f32 / reach_y as f32;
            let t = tx.max(ty);
            let alpha = (1.0 - t).powf(1.6) * strength * 0.9;
            if alpha > 0.01 {
                blend_pixel(img, x, y, rgba_alpha(r, g, b, alpha));
            }
        }
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
            backdrop: WatermarkBackdrop::None,
            backdrop_opacity_percent: 65,
            backdrop_padding: 14,
            backdrop_color: "#000000".to_string(),
            backdrop_logo_offset_x: 14,
            backdrop_logo_offset_y: 14,
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
            backdrop: WatermarkBackdrop::DarkPill,
            backdrop_opacity_percent: 65,
            backdrop_padding: 14,
            backdrop_color: "#000000".to_string(),
            backdrop_logo_offset_x: 14,
            backdrop_logo_offset_y: 14,
        };
        let (wm_w, wm_h) = watermark_dimensions(canvas, &config, 100, 50);
        assert_eq!(wm_w, 240);
        assert_eq!(wm_h, 120);
    }
}
