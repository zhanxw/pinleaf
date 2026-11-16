use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use gpui::RenderImage;
use image::{Frame, RgbaImage};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

pub const APP_ICON_SVG: &str = include_str!("../../../assets/icon.svg");

pub fn render_icon_pixmap(size_px: u32) -> Result<Pixmap> {
    if size_px == 0 {
        return Err(anyhow!("icon size must be positive"));
    }
    let tree = Tree::from_str(APP_ICON_SVG, &Options::default())
        .context("failed to parse embedded icon SVG")?;
    let mut pixmap = Pixmap::new(size_px, size_px)
        .ok_or_else(|| anyhow!("failed to allocate {size_px}x{size_px} pixmap"))?;
    let scale = size_px as f32 / 1024.0;
    resvg::render(&tree, Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Ok(pixmap)
}

pub fn render_icon_png_bytes(size_px: u32) -> Result<Vec<u8>> {
    let pixmap = render_icon_pixmap(size_px)?;
    pixmap
        .encode_png()
        .context("failed to encode icon pixmap as PNG")
}

pub fn build_gpui_app_icon(size_px: u32) -> Arc<RenderImage> {
    let pixmap = render_icon_pixmap(size_px).expect("embedded Pinleaf SVG icon renders");
    let mut bytes_bgra = pixmap.take();
    // tiny-skia renders premultiplied RGBA; GPUI/Metal RenderImage expects premultiplied BGRA.
    for px in bytes_bgra.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let rgba_img = RgbaImage::from_raw(size_px, size_px, bytes_bgra)
        .expect("icon buffer dimensions match pixmap");
    Arc::new(RenderImage::new(vec![Frame::new(rgba_img)]))
}

pub fn install_macos_dock_icon() {
    #[cfg(target_os = "macos")]
    {
        if std::env::current_exe()
            .ok()
            .is_some_and(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        {
            return;
        }
        use objc2::{AnyThread, MainThreadMarker};
        use objc2_app_kit::{NSApplication, NSImage};
        use objc2_foundation::NSData;

        let Ok(png) = render_icon_png_bytes(512) else {
            return;
        };
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let data = NSData::with_bytes(&png);
        if let Some(ns_image) = NSImage::initWithData(NSImage::alloc(), &data) {
            unsafe {
                NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&ns_image));
            }
        }
    }
}

pub fn export_icon_bundle(assets_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(assets_dir)?;
    std::fs::write(assets_dir.join("icon_1024.png"), render_icon_png_bytes(1024)?)?;
    std::fs::write(assets_dir.join("icon_256.png"), render_icon_png_bytes(256)?)?;

    #[cfg(target_os = "macos")]
    {
        let iconset_dir = assets_dir.join("Pinleaf.iconset");
        if iconset_dir.exists() {
            std::fs::remove_dir_all(&iconset_dir)?;
        }
        std::fs::create_dir_all(&iconset_dir)?;

        let specs = [
            ("icon_16x16.png", 16),
            ("icon_16x16@2x.png", 32),
            ("icon_32x32.png", 32),
            ("icon_32x32@2x.png", 64),
            ("icon_128x128.png", 128),
            ("icon_128x128@2x.png", 256),
            ("icon_256x256.png", 256),
            ("icon_256x256@2x.png", 512),
            ("icon_512x512.png", 512),
            ("icon_512x512@2x.png", 1024),
        ];

        for (filename, size_px) in specs {
            std::fs::write(iconset_dir.join(filename), render_icon_png_bytes(size_px)?)?;
        }

        let icns_path = assets_dir.join("Pinleaf.icns");
        let status = std::process::Command::new("/usr/bin/iconutil")
            .args(["-c", "icns"])
            .arg(&iconset_dir)
            .arg("-o")
            .arg(&icns_path)
            .status()
            .context("failed to execute /usr/bin/iconutil")?;

        std::fs::remove_dir_all(&iconset_dir)?;

        if !status.success() {
            return Err(anyhow!("iconutil exited with status {status}"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_renders_at_hero_and_toolbar_sizes() {
        let hero = render_icon_pixmap(1024).expect("1024px icon renders");
        let small = render_icon_pixmap(32).expect("32px icon renders");
        assert_eq!(hero.width(), 1024);
        assert_eq!(hero.height(), 1024);
        assert_eq!(small.width(), 32);
        assert_eq!(small.height(), 32);

        let corner = hero.pixel(8, 8).expect("corner pixel in bounds");
        let center = hero.pixel(512, 512).expect("center pixel in bounds");
        assert_eq!(corner.alpha(), 0);
        assert_eq!(center.alpha(), 255);
    }

    #[test]
    fn icon_png_encoding_produces_valid_png_header() {
        let png = render_icon_png_bytes(128).expect("128px PNG renders");
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(png.len() > 256);
    }
}
