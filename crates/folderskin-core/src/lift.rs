//! A picture's subject lifted off whatever it was painted on, by the system's own model: on macOS
//! 14 and later, Vision's foreground instance mask, the one Preview and Photos lift a subject
//! with.
//!
//! A free icon is asked for on a flat key colour, and a model keeps it about half the time.
//! klein turns magenta into a lavender or rose studio sweep with a glow and a soft floor, and a
//! colour key can't then tell the sweep from a white robot lit lavender by it, a grey camera in a
//! fox's paws, or the wash a watercolour cactus sits in. Vision tells them apart by what the
//! subject is, not its colour: in testing it lifted all seven of klein's icons whole, in about a
//! quarter of a second each.
//!
//! Elsewhere, and on a picture where it finds no subject, [`subject_mask`] is `None`, and the
//! caller keys the backdrop out instead.

use image::{GrayImage, RgbaImage};

/// How much each pixel of `img` belongs to its subject: 255 on it, 0 around it, soft along its
/// edge, the same size as `img`. `None` when the system can't lift subjects, or finds none.
pub fn subject_mask(img: &RgbaImage) -> Option<GrayImage> {
    imp::subject_mask(img)
}

#[cfg(target_os = "macos")]
mod imp {
    use image::{imageops, GrayImage, Luma, RgbaImage};
    use objc2::rc::autoreleasepool;
    use objc2::runtime::AnyClass;
    use objc2::AnyThread;
    use objc2_core_video::{
        kCVPixelFormatType_OneComponent32Float, kCVReturnSuccess, CVPixelBuffer,
        CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
        CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress,
        CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
    };
    use objc2_foundation::{NSArray, NSData, NSDictionary};
    use objc2_vision::{VNGenerateForegroundInstanceMaskRequest, VNImageRequestHandler, VNRequest};

    pub fn subject_mask(img: &RgbaImage) -> Option<GrayImage> {
        // New in macOS 14: before that the class isn't there to ask.
        AnyClass::get(c"VNGenerateForegroundInstanceMaskRequest")?;
        let png = crate::raster::encode_png(img);
        let mask = autoreleasepool(|_| {
            let data = NSData::with_bytes(&png);
            let handler = VNImageRequestHandler::initWithData_options(
                VNImageRequestHandler::alloc(),
                &data,
                &NSDictionary::new(),
            );
            // SAFETY: a plain new request, performed once, below.
            let request = unsafe { VNGenerateForegroundInstanceMaskRequest::new() };
            let as_request: &VNRequest = &request;
            handler
                .performRequests_error(&NSArray::from_slice(&[as_request]))
                .ok()?;
            // SAFETY: the request has run, so its results are its own observations.
            let observation = unsafe { request.results() }?.iter().next()?;
            // SAFETY: the observation came from this handler's request.
            let instances = unsafe { observation.allInstances() };
            if instances.count() == 0 {
                return None;
            }
            // SAFETY: as above; the mask is one 32-bit float per pixel, the image's size.
            let buffer = unsafe {
                observation
                    .generateScaledMaskForImageForInstances_fromRequestHandler_error(
                        &instances, &handler,
                    )
                    .ok()?
            };
            read_mask(&buffer)
        })?;
        let (w, h) = img.dimensions();
        Some(if mask.dimensions() == (w, h) {
            mask
        } else {
            imageops::resize(&mask, w, h, imageops::FilterType::Triangle)
        })
    }

    /// A one-channel float pixel buffer as 8-bit greys.
    fn read_mask(buffer: &CVPixelBuffer) -> Option<GrayImage> {
        if CVPixelBufferGetPixelFormatType(buffer) != kCVPixelFormatType_OneComponent32Float {
            return None;
        }
        // SAFETY: locked read-only for the reads below and unlocked with the same flag after.
        if unsafe { CVPixelBufferLockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) }
            != kCVReturnSuccess
        {
            return None;
        }
        let (w, h) = (
            CVPixelBufferGetWidth(buffer),
            CVPixelBufferGetHeight(buffer),
        );
        let stride = CVPixelBufferGetBytesPerRow(buffer);
        let base = CVPixelBufferGetBaseAddress(buffer)
            .cast::<u8>()
            .cast_const();
        let mask = (!base.is_null() && w > 0 && h > 0 && stride >= w * 4).then(|| {
            GrayImage::from_fn(w as u32, h as u32, |x, y| {
                // SAFETY: inside the locked buffer: row `y` of `h`, float `x` of `w`, and rows
                // `stride` bytes apart, each at least `w` floats long.
                let v = unsafe {
                    base.add(y as usize * stride)
                        .cast::<f32>()
                        .add(x as usize)
                        .read_unaligned()
                };
                Luma([(v.clamp(0.0, 1.0) * 255.0).round() as u8])
            })
        });
        // SAFETY: the lock taken above.
        unsafe { CVPixelBufferUnlockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) };
        mask
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use image::{GrayImage, RgbaImage};

    pub fn subject_mask(_img: &RgbaImage) -> Option<GrayImage> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// klein's clay fox, shrunk: a grey camera in its paws and a rose studio sweep around it,
    /// which a colour key eats the camera with.
    #[test]
    #[ignore = "needs Vision's subject lifting: macOS 14 or later, on a Mac rather than a VM"]
    fn vision_lifts_the_whole_fox_and_none_of_its_backdrop() {
        let img = image::open(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lift/fox.jpg"),
        )
        .unwrap()
        .to_rgba8();
        let mask = subject_mask(&img).expect("a subject");
        let (w, h) = img.dimensions();
        assert_eq!(mask.dimensions(), (w, h));
        for (x, y) in [
            (2, 2),
            (w - 3, 2),
            (2, h - 3),
            (w - 3, h - 3),
            (w / 8, h / 2),
        ] {
            assert!(mask.get_pixel(x, y).0[0] < 16, "backdrop at {x},{y}");
        }
        // The camera, the face and the tail.
        for (x, y) in [
            (w / 2, h * 55 / 100),
            (w / 2, h * 3 / 10),
            (w * 70 / 100, h * 62 / 100),
        ] {
            assert!(mask.get_pixel(x, y).0[0] > 240, "subject at {x},{y}");
        }
    }
}
