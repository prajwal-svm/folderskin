//! Windows `.ico` container with PNG-compressed entries.
//!
//! An ICO is an `ICONDIR` header, one 16-byte `ICONDIRENTRY` per image, then the image data.
//! Everything is little-endian. Since Vista every entry may be a whole PNG file rather than a
//! BMP, which is what we write: the icon data is exactly the PNG bytes the compositor produced.

/// Why an `.ico` could not be read.
#[derive(Debug, thiserror::Error)]
pub enum IcoError {
    #[error("not an .ico file")]
    NotAnIcon,
    #[error(".ico file is truncated")]
    Truncated,
}

/// Bytes of the `ICONDIR` header.
const HEADER_LEN: usize = 6;
/// Bytes of one `ICONDIRENTRY`.
const ENTRY_LEN: usize = 16;

/// Packs `(size, png bytes)` entries into an `.ico`.
///
/// The directory addresses an image's width in a single byte, so entries are limited to 1..=256
/// px (256 is stored as 0). Anything outside that range is skipped rather than truncated into a
/// wrong-sized entry, so callers can hand over the whole icon ladder and get the part Windows
/// can actually hold.
pub fn write_ico(entries: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let usable: Vec<(u32, &[u8])> = entries
        .iter()
        .filter(|(size, _)| (1..=256).contains(size))
        .map(|(size, png)| (*size, png.as_slice()))
        .collect();

    let data_len: usize = usable.iter().map(|(_, png)| png.len()).sum();
    let mut out = Vec::with_capacity(HEADER_LEN + usable.len() * ENTRY_LEN + data_len);

    // ICONDIR: reserved, type (1 = icon), image count.
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(usable.len() as u16).to_le_bytes());

    let mut offset = HEADER_LEN + usable.len() * ENTRY_LEN;
    for (size, png) in &usable {
        let dim = if *size == 256 { 0u8 } else { *size as u8 };
        out.push(dim); // width
        out.push(dim); // height
        out.push(0); // palette colours (0 = no palette)
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in &usable {
        out.extend_from_slice(png);
    }
    out
}

/// Reads the directory of an `.ico` as `(width, byte length, offset)` per entry.
///
/// A width byte of 0 means 256 px. Every entry's data range is checked against `bytes`, so a
/// successful read means the offsets can be sliced.
pub fn read_ico_header(bytes: &[u8]) -> Result<Vec<(u32, u32, u32)>, IcoError> {
    if bytes.len() < HEADER_LEN {
        return Err(IcoError::Truncated);
    }
    let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32_at =
        |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    if u16_at(0) != 0 || u16_at(2) != 1 {
        return Err(IcoError::NotAnIcon);
    }
    let count = u16_at(4) as usize;
    if bytes.len() < HEADER_LEN + count * ENTRY_LEN {
        return Err(IcoError::Truncated);
    }
    let mut dir = Vec::with_capacity(count);
    for i in 0..count {
        let e = HEADER_LEN + i * ENTRY_LEN;
        let width = match bytes[e] {
            0 => 256,
            w => u32::from(w),
        };
        let len = u32_at(e + 8);
        let offset = u32_at(e + 12);
        let end = (offset as usize)
            .checked_add(len as usize)
            .ok_or(IcoError::Truncated)?;
        if end > bytes.len() {
            return Err(IcoError::Truncated);
        }
        dir.push((width, len, offset));
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(size: u32) -> Vec<u8> {
        crate::raster::encode_png(&image::RgbaImage::from_pixel(
            size,
            size,
            image::Rgba([0, 0, 0, 255]),
        ))
    }

    #[test]
    fn ico_header_roundtrip() {
        let png16 = png(16);
        let png256 = png(256);
        let ico = write_ico(&[(16, png16.clone()), (256, png256.clone())]);
        assert_eq!(&ico[0..6], &[0, 0, 1, 0, 2, 0]);
        let dir = read_ico_header(&ico).unwrap();
        assert_eq!(dir[0], (16, png16.len() as u32, 6 + 32));
        assert_eq!(dir[1].0, 256); // stored as 0 in the byte, decoded as 256
        assert_eq!(
            &ico[dir[1].2 as usize..dir[1].2 as usize + 8],
            &png256[0..8]
        );
    }

    #[test]
    fn every_entry_decodes_back_at_its_declared_size() {
        let sizes = [16u32, 24, 32, 48, 64, 128, 256];
        let entries: Vec<(u32, Vec<u8>)> = sizes.iter().map(|&s| (s, png(s))).collect();
        let ico = write_ico(&entries);
        let dir = read_ico_header(&ico).unwrap();
        assert_eq!(dir.len(), sizes.len());
        for (i, &size) in sizes.iter().enumerate() {
            let (width, len, offset) = dir[i];
            assert_eq!(width, size);
            let data = &ico[offset as usize..offset as usize + len as usize];
            let img = image::load_from_memory(data).unwrap();
            assert_eq!((img.width(), img.height()), (size, size), "{size}");
        }
        // Offsets follow the directory, contiguously, in order.
        assert_eq!(dir[0].2 as usize, 6 + sizes.len() * 16);
        for pair in dir.windows(2) {
            assert_eq!(pair[1].2, pair[0].2 + pair[0].1);
        }
        assert_eq!(
            dir.last().map(|(_, l, o)| (o + l) as usize),
            Some(ico.len())
        );
    }

    #[test]
    fn entry_fields_say_32bpp_one_plane() {
        let ico = write_ico(&[(32, png(32))]);
        let e = 6;
        assert_eq!((ico[e], ico[e + 1]), (32, 32)); // width, height
        assert_eq!((ico[e + 2], ico[e + 3]), (0, 0)); // palette, reserved
        assert_eq!(u16::from_le_bytes([ico[e + 4], ico[e + 5]]), 1); // planes
        assert_eq!(u16::from_le_bytes([ico[e + 6], ico[e + 7]]), 32); // bpp
    }

    #[test]
    fn sizes_the_format_cannot_address_are_skipped() {
        let ico = write_ico(&[(2048, png(16)), (256, png(256)), (0, vec![1, 2, 3])]);
        let dir = read_ico_header(&ico).unwrap();
        assert_eq!(dir.len(), 1);
        assert_eq!(dir[0].0, 256);
        assert_eq!(u16::from_le_bytes([ico[4], ico[5]]), 1);
    }

    #[test]
    fn empty_input_is_a_valid_empty_icon() {
        let ico = write_ico(&[]);
        assert_eq!(ico, vec![0, 0, 1, 0, 0, 0]);
        assert!(read_ico_header(&ico).unwrap().is_empty());
    }

    #[test]
    fn garbage_and_truncation_are_rejected() {
        assert!(matches!(read_ico_header(&[0, 0]), Err(IcoError::Truncated)));
        assert!(matches!(
            read_ico_header(&[0x89, b'P', b'N', b'G', 0, 0]),
            Err(IcoError::NotAnIcon)
        ));
        // Claims two entries but carries neither.
        assert!(matches!(
            read_ico_header(&[0, 0, 1, 0, 2, 0]),
            Err(IcoError::Truncated)
        ));
        // Entry whose data runs past the end of the file.
        let mut ico = write_ico(&[(16, png(16))]);
        ico.truncate(ico.len() - 4);
        assert!(matches!(read_ico_header(&ico), Err(IcoError::Truncated)));
    }
}
