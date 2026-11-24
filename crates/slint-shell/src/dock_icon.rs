//! Dock icon: a bare executable shows macOS's generic tile unless the
//! process installs one, so hand the Dock the bundled logo at startup.

pub fn render_logo_png(px: u32) -> Option<Vec<u8>> {
    let svg = include_bytes!("../../../assets/logo.svg");
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default()).ok()?;
    let scale = px as f32 / tree.size().width();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().ok()
}

#[cfg(target_os = "macos")]
pub fn apply_logo() {
    use objc2::MainThreadMarker;
    use objc2::AnyThread;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{NSData, NSUInteger};

    let Some(mtm) = MainThreadMarker::new() else {
        logging::warn!("dock icon: not on the main thread, skipped");
        return;
    };
    let Some(png) = render_logo_png(1024) else {
        logging::warn!("dock icon: logo failed to render");
        return;
    };
    let data =
        unsafe { NSData::dataWithBytes_length(png.as_ptr().cast(), png.len() as NSUInteger) };
    let Some(icon) = NSImage::initWithData(NSImage::alloc(), &data) else {
        logging::warn!("dock icon: PNG did not decode into an NSImage");
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    unsafe { app.setApplicationIconImage(Some(&icon)) };
    logging::info!("milestone: dock icon applied");
}

#[cfg(not(target_os = "macos"))]
pub fn apply_logo() {}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::render_logo_png;

    #[test]
    fn logo_renders_as_square_png() {
        let png = render_logo_png(1024).expect("bundled logo must rasterize");
        std::fs::write("/tmp/magma_logo_render.png", &png).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 1024);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 1024);
    }
}
