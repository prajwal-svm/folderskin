//! Unpacking the zipped runtime: just enough of the zip format for release archives (stored and
//! deflated entries, zip64 sizes), with every entry's CRC checked and no entry allowed outside
//! the folder it is unpacked into.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

/// One file in an archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The name inside the archive, with `/` between folders.
    pub name: String,
    method: u16,
    crc: u32,
    compressed: u64,
    size: u64,
    header: u64,
    flags: u16,
    /// Unix permissions, when the archive was made on a system that has them.
    mode: Option<u32>,
}

impl Entry {
    pub fn is_dir(&self) -> bool {
        self.name.ends_with('/')
    }
}

fn bad(why: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, why.into())
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap_or_default())
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap_or_default())
}

/// The entries of the zip at `path`, from its central directory.
pub fn entries(file: &mut File) -> std::io::Result<Vec<Entry>> {
    let len = file.seek(SeekFrom::End(0))?;
    // The end-of-central-directory record is 22 bytes plus a comment of up to 64 KB.
    let tail_len = len.min(22 + 65_535);
    file.seek(SeekFrom::Start(len - tail_len))?;
    let mut tail = vec![0u8; tail_len as usize];
    file.read_exact(&mut tail)?;
    let eocd = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|&i| u32_at(&tail, i) == 0x0605_4b50)
        .ok_or_else(|| bad("it isn't a zip archive"))?;
    let mut count = u16_at(&tail, eocd + 10) as u64;
    let mut dir_size = u32_at(&tail, eocd + 12) as u64;
    let mut dir_at = u32_at(&tail, eocd + 16) as u64;
    if count == 0xFFFF || dir_size == 0xFFFF_FFFF || dir_at == 0xFFFF_FFFF {
        // Zip64: a locator just before the record says where the bigger record is.
        let locator = eocd
            .checked_sub(20)
            .filter(|&i| u32_at(&tail, i) == 0x0706_4b50)
            .ok_or_else(|| bad("its zip64 directory is missing"))?;
        let record_at = u64_at(&tail, locator + 8);
        let mut record = [0u8; 56];
        file.seek(SeekFrom::Start(record_at))?;
        file.read_exact(&mut record)?;
        if u32_at(&record, 0) != 0x0606_4b50 {
            return Err(bad("its zip64 directory is damaged"));
        }
        count = u64_at(&record, 32);
        dir_size = u64_at(&record, 40);
        dir_at = u64_at(&record, 48);
    }
    if dir_at.saturating_add(dir_size) > len {
        return Err(bad("its directory points past its end"));
    }
    file.seek(SeekFrom::Start(dir_at))?;
    let mut dir = vec![0u8; dir_size as usize];
    file.read_exact(&mut dir)?;

    let mut out = Vec::with_capacity(count as usize);
    let mut at = 0usize;
    for _ in 0..count {
        if at + 46 > dir.len() || u32_at(&dir, at) != 0x0201_4b50 {
            return Err(bad("its directory is damaged"));
        }
        let made_by = u16_at(&dir, at + 4) >> 8;
        let flags = u16_at(&dir, at + 8);
        let method = u16_at(&dir, at + 10);
        let crc = u32_at(&dir, at + 16);
        let mut compressed = u32_at(&dir, at + 20) as u64;
        let mut size = u32_at(&dir, at + 24) as u64;
        let name_len = u16_at(&dir, at + 28) as usize;
        let extra_len = u16_at(&dir, at + 30) as usize;
        let comment_len = u16_at(&dir, at + 32) as usize;
        let external = u32_at(&dir, at + 38);
        let mut header = u32_at(&dir, at + 42) as u64;
        let name_at = at + 46;
        let extra_at = name_at + name_len;
        let next = extra_at + extra_len + comment_len;
        if next > dir.len() {
            return Err(bad("its directory is damaged"));
        }
        let name = String::from_utf8_lossy(&dir[name_at..extra_at]).into_owned();
        // The zip64 extra field holds, in this order, whichever of the three didn't fit.
        let mut extra = &dir[extra_at..extra_at + extra_len];
        while extra.len() >= 4 {
            let (id, len) = (u16_at(extra, 0), u16_at(extra, 2) as usize);
            let data = &extra[4..(4 + len).min(extra.len())];
            if id == 0x0001 {
                let mut field = data
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|c| u64::from_le_bytes(*c));
                if size == 0xFFFF_FFFF {
                    size = field.next().ok_or_else(|| bad("a zip64 size is missing"))?;
                }
                if compressed == 0xFFFF_FFFF {
                    compressed = field.next().ok_or_else(|| bad("a zip64 size is missing"))?;
                }
                if header == 0xFFFF_FFFF {
                    header = field
                        .next()
                        .ok_or_else(|| bad("a zip64 offset is missing"))?;
                }
            }
            extra = &extra[(4 + len).min(extra.len())..];
        }
        // Made on Unix (3) or macOS (19): the high half of the external attributes is the mode.
        let mode = matches!(made_by, 3 | 19)
            .then_some(external >> 16)
            .filter(|&m| m != 0);
        out.push(Entry {
            name,
            method,
            crc,
            compressed,
            size,
            header,
            flags,
            mode,
        });
        at = next;
    }
    Ok(out)
}

/// Where `name` goes inside `dest`, or `None` when it would land outside it.
fn inside(dest: &Path, name: &str) -> Option<PathBuf> {
    let relative = Path::new(name);
    let safe = relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    (safe && !name.is_empty()).then(|| dest.join(relative))
}

/// Copies one entry's bytes to `out`, inflating them if they are deflated, and checks its CRC.
pub fn copy_entry(file: &mut File, entry: &Entry, out: &mut impl Write) -> std::io::Result<()> {
    if entry.flags & 1 != 0 {
        return Err(bad(format!("{} is encrypted", entry.name)));
    }
    let mut local = [0u8; 30];
    file.seek(SeekFrom::Start(entry.header))?;
    file.read_exact(&mut local)?;
    if u32_at(&local, 0) != 0x0403_4b50 {
        return Err(bad(format!("{} is damaged", entry.name)));
    }
    let skip = u16_at(&local, 26) as i64 + u16_at(&local, 28) as i64;
    file.seek(SeekFrom::Current(skip))?;
    let raw = Read::by_ref(file).take(entry.compressed);
    let mut reader: Box<dyn Read + '_> = match entry.method {
        0 => Box::new(raw),
        8 => Box::new(flate2::read::DeflateDecoder::new(raw)),
        other => {
            return Err(bad(format!(
                "{} is compressed a way this can't unpack ({other})",
                entry.name
            )))
        }
    };
    let mut crc = crc32fast::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut written = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        crc.update(&buf[..n]);
        out.write_all(&buf[..n])?;
        written += n as u64;
    }
    if written != entry.size || crc.finalize() != entry.crc {
        return Err(bad(format!("{} doesn't match its checksum", entry.name)));
    }
    Ok(())
}

/// Unpacks every entry `pick` wants into `dest`: `pick` gets the entry's name and returns the
/// path, relative to `dest`, it should have, or `None` to leave it out. Returns what was written.
pub fn extract(
    zip: &Path,
    dest: &Path,
    pick: impl Fn(&str) -> Option<String>,
) -> std::io::Result<Vec<PathBuf>> {
    let mut file = File::open(zip)?;
    let mut written = Vec::new();
    for entry in entries(&mut file)? {
        if entry.is_dir() {
            continue;
        }
        let Some(name) = pick(&entry.name) else {
            continue;
        };
        let target = inside(dest, &name)
            .ok_or_else(|| bad(format!("{} would unpack outside its folder", entry.name)))?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&target)?;
        copy_entry(&mut file, &entry, &mut out)?;
        out.flush()?;
        #[cfg(unix)]
        if let Some(mode) = entry.mode {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode & 0o777));
        }
        written.push(target);
    }
    Ok(written)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A zip of `files`, deflated or stored, the way release archives are made.
    pub(crate) fn zip(files: &[(&str, &[u8], bool)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data, deflate) in files {
            let crc = crc32fast::hash(data);
            let body = if *deflate {
                let mut enc =
                    flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                enc.write_all(data).unwrap();
                enc.finish().unwrap()
            } else {
                data.to_vec()
            };
            let method: u16 = if *deflate { 8 } else { 0 };
            let offset = out.len() as u32;
            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&[20, 0, 0, 0]);
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&[0; 4]);
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&[0, 0]);
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&body);

            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&[20, 3, 20, 0, 0, 0]); // made on Unix
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&[0; 4]);
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&(body.len() as u32).to_le_bytes());
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0; 8]);
            central.extend_from_slice(&(0o100755u32 << 16).to_le_bytes());
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let dir_at = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&dir_at.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fs-unzip-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn stored_and_deflated_entries_unpack_as_they_went_in() {
        let dir = temp_dir("both");
        let big: Vec<u8> = (0..200_000u32).map(|i| (i % 97) as u8).collect();
        std::fs::write(
            dir.join("a.zip"),
            zip(&[
                ("sd-cli", b"#!/bin/sh\necho hi\n", false),
                ("lib/ggml.dll", &big, true),
                ("docs/", b"", false),
            ]),
        )
        .unwrap();
        let out = dir.join("out");
        let written = extract(&dir.join("a.zip"), &out, |n| Some(n.to_string())).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!(
            std::fs::read(out.join("sd-cli")).unwrap(),
            b"#!/bin/sh\necho hi\n"
        );
        assert_eq!(
            std::fs::read(out.join("lib").join("ggml.dll")).unwrap(),
            big
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(out.join("sd-cli"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o755);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn entries_can_be_picked_and_flattened() {
        let dir = temp_dir("pick");
        std::fs::write(
            dir.join("webp.zip"),
            zip(&[
                ("libwebp-1.6.0-windows-x64/bin/cwebp.exe", b"cwebp", true),
                ("libwebp-1.6.0-windows-x64/bin/dwebp.exe", b"dwebp", true),
                ("libwebp-1.6.0-windows-x64/README", b"read me", true),
            ]),
        )
        .unwrap();
        let written = extract(&dir.join("webp.zip"), &dir, |n| {
            n.ends_with("/bin/cwebp.exe")
                .then(|| n.rsplit('/').next().unwrap().to_string())
        })
        .unwrap();
        assert_eq!(written, [dir.join("cwebp.exe")]);
        assert_eq!(std::fs::read(dir.join("cwebp.exe")).unwrap(), b"cwebp");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_entry_that_climbs_out_is_refused() {
        let dir = temp_dir("slip");
        std::fs::write(
            dir.join("evil.zip"),
            zip(&[("../escaped.txt", b"x", false)]),
        )
        .unwrap();
        let err = extract(&dir.join("evil.zip"), &dir.join("out"), |n| {
            Some(n.to_string())
        })
        .unwrap_err();
        assert!(err.to_string().contains("outside"), "{err}");
        assert!(!dir.join("escaped.txt").exists());
        assert!(inside(&dir, "/etc/passwd").is_none());
        assert!(inside(&dir, "C:/Windows/x").is_none() || cfg!(not(windows)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A real release archive, when `FOLDERSKIN_UNZIP_SAMPLE` names one (stable-diffusion.cpp's
    /// or libwebp's zip from the downloads folder): every entry unpacks and matches its CRC.
    #[test]
    fn a_real_release_archive_unpacks() {
        let Some(sample) = std::env::var_os("FOLDERSKIN_UNZIP_SAMPLE") else {
            return;
        };
        let dir = temp_dir("sample");
        let written = extract(Path::new(&sample), &dir, |n| Some(n.to_string())).unwrap();
        assert!(!written.is_empty());
        let mut file = File::open(&sample).unwrap();
        let files = entries(&mut file)
            .unwrap()
            .into_iter()
            .filter(|e| !e.is_dir())
            .count();
        assert_eq!(written.len(), files);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn damage_is_caught() {
        let dir = temp_dir("damaged");
        let mut bytes = zip(&[("a.txt", b"hello hello hello hello", false)]);
        // Flip a byte of the stored data: the CRC no longer matches.
        let at = bytes.windows(5).position(|w| w == b"hello").unwrap();
        bytes[at] ^= 1;
        std::fs::write(dir.join("bad.zip"), &bytes).unwrap();
        let err = extract(&dir.join("bad.zip"), &dir.join("out"), |n| {
            Some(n.to_string())
        })
        .unwrap_err();
        assert!(err.to_string().contains("checksum"), "{err}");
        std::fs::write(dir.join("not.zip"), b"not a zip at all").unwrap();
        let err = extract(&dir.join("not.zip"), &dir, |n| Some(n.to_string())).unwrap_err();
        assert!(err.to_string().contains("isn't a zip"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
