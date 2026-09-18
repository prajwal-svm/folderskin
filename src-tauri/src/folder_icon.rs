//! The icon a folder currently shows in the OS file manager, as PNG bytes.
//!
//! macOS asks NSWorkspace (so a folder that already has a custom icon shows it); other
//! platforms fall back to the compositor's plain default folder.

use std::path::Path;

/// PNG bytes of the folder's current icon at about `size` px, or None if the OS cannot say.
#[cfg(target_os = "macos")]
pub fn current_icon_png(folder: &Path, size: u32) -> Option<Vec<u8>> {
    use objc2::AnyThread;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSGraphicsContext, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    let path = NSString::from_str(&folder.to_string_lossy());
    let image = NSWorkspace::sharedWorkspace().iconForFile(&path);
    // Draw the icon into a fresh bitmap at the requested pixel size so we control the output.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            size as isize,
            size as isize,
            8,
            4,
            true,
            false,
            objc2_app_kit::NSDeviceRGBColorSpace,
            0,
            0,
        )
    }?;
    let ctx = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
    {
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&ctx));
        image.drawInRect_fromRect_operation_fraction(
            NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(size as f64, size as f64),
            ),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
            objc2_app_kit::NSCompositingOperation::SourceOver,
            1.0,
        );
        NSGraphicsContext::restoreGraphicsState_class();
    }
    let data = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(data.to_vec())
}

#[cfg(not(target_os = "macos"))]
pub fn current_icon_png(_folder: &Path, _size: u32) -> Option<Vec<u8>> {
    None
}

/// Shows the app's own icon in the Dock even for the unbundled dev binary.
#[cfg(target_os = "macos")]
pub fn set_dock_icon(png: &[u8]) {
    use objc2::AnyThread;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(png);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_dock_icon(_png: &[u8]) {}
