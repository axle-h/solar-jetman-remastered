//! Reading a ROM the player names: a `.nes` file, or a `.zip` holding exactly one.

/// Largest ROM file accepted, bytes. The real one is 256 KB.
const MAX_SIZE: usize = 8 << 20;

fn u16_at(b: &[u8], at: usize) -> Option<usize> {
    Some(usize::from(u16::from_le_bytes(
        b.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn u32_at(b: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?) as usize)
}

/// The ROM file's bytes, unpacked when `file` is a zip archive.
pub fn rom_bytes(file: Vec<u8>) -> Result<Vec<u8>, String> {
    if file.starts_with(b"PK\x03\x04") {
        extract_nes(&file)
    } else {
        Ok(file)
    }
}

const BAD_ZIP: &str = "the zip file is damaged or uses an unsupported format";

/// The one `.nes` entry of a zip archive (stored or deflated).
fn extract_nes(zip: &[u8]) -> Result<Vec<u8>, String> {
    let end = (0..zip.len().saturating_sub(21))
        .rev()
        .take(70_000)
        .find(|&i| zip[i..].starts_with(b"PK\x05\x06"))
        .ok_or(BAD_ZIP)?;
    let count = u16_at(zip, end + 10).ok_or(BAD_ZIP)?;
    let mut at = u32_at(zip, end + 16).ok_or(BAD_ZIP)?;
    let mut found = Vec::new();
    for _ in 0..count {
        if !zip.get(at..).is_some_and(|r| r.starts_with(b"PK\x01\x02")) {
            return Err(BAD_ZIP.into());
        }
        let method = u16_at(zip, at + 10).ok_or(BAD_ZIP)?;
        let packed = u32_at(zip, at + 20).ok_or(BAD_ZIP)?;
        let size = u32_at(zip, at + 24).ok_or(BAD_ZIP)?;
        let (name_len, extra, comment) = (
            u16_at(zip, at + 28).ok_or(BAD_ZIP)?,
            u16_at(zip, at + 30).ok_or(BAD_ZIP)?,
            u16_at(zip, at + 32).ok_or(BAD_ZIP)?,
        );
        let local = u32_at(zip, at + 42).ok_or(BAD_ZIP)?;
        let name = zip.get(at + 46..at + 46 + name_len).ok_or(BAD_ZIP)?;
        if name.len() > 4 && name[name.len() - 4..].eq_ignore_ascii_case(b".nes") {
            found.push((method, packed, size, local));
        }
        at += 46 + name_len + extra + comment;
    }
    let [(method, packed, size, local)] = found[..] else {
        return Err(if found.is_empty() {
            "the zip file holds no .nes file".into()
        } else {
            "the zip file holds more than one .nes file".into()
        });
    };
    if size > MAX_SIZE {
        return Err("the .nes file in the zip is too large".into());
    }
    if !zip
        .get(local..)
        .is_some_and(|r| r.starts_with(b"PK\x03\x04"))
    {
        return Err(BAD_ZIP.into());
    }
    let start = local
        + 30
        + u16_at(zip, local + 26).ok_or(BAD_ZIP)?
        + u16_at(zip, local + 28).ok_or(BAD_ZIP)?;
    let data = zip.get(start..start + packed).ok_or(BAD_ZIP)?;
    let out = match method {
        0 => data.to_vec(),
        8 => miniz_oxide::inflate::decompress_to_vec_with_limit(data, MAX_SIZE)
            .map_err(|_| BAD_ZIP.to_owned())?,
        _ => return Err(BAD_ZIP.into()),
    };
    if out.len() != size {
        return Err(BAD_ZIP.into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-entry zip archive (checksums left zero; the reader does not use them).
    fn zip(name: &str, data: &[u8], deflate: bool) -> Vec<u8> {
        let packed = if deflate {
            miniz_oxide::deflate::compress_to_vec(data, 6)
        } else {
            data.to_vec()
        };
        let method: u16 = if deflate { 8 } else { 0 };
        let mut out = Vec::new();
        let header = |out: &mut Vec<u8>, central: bool| {
            out.extend(if central {
                b"PK\x01\x02".as_slice()
            } else {
                b"PK\x03\x04"
            });
            if central {
                out.extend([20, 0]);
            }
            out.extend([20, 0, 0, 0]);
            out.extend(method.to_le_bytes());
            out.extend([0; 8]);
            out.extend((packed.len() as u32).to_le_bytes());
            out.extend((data.len() as u32).to_le_bytes());
            out.extend((name.len() as u16).to_le_bytes());
            out.extend([0, 0]);
            if central {
                out.extend([0; 14]);
            }
            out.extend(name.as_bytes());
        };
        header(&mut out, false);
        out.extend(&packed);
        let directory = out.len();
        header(&mut out, true);
        let size = out.len() - directory;
        out.extend(b"PK\x05\x06\0\0\0\0\x01\0\x01\0");
        out.extend((size as u32).to_le_bytes());
        out.extend((directory as u32).to_le_bytes());
        out.extend([0, 0]);
        out
    }

    #[test]
    fn plain_files_pass_through() {
        assert_eq!(rom_bytes(vec![1, 2, 3]).unwrap(), [1, 2, 3]);
    }

    #[test]
    fn zip_entries_are_unpacked() {
        let data: Vec<u8> = (0..5000u32).map(|i| (i % 7) as u8).collect();
        for deflate in [false, true] {
            let archive = zip("Game (USA).NES", &data, deflate);
            assert_eq!(rom_bytes(archive).unwrap(), data);
        }
    }

    #[test]
    fn zips_without_a_rom_are_refused() {
        let archive = zip("readme.txt", b"hi", false);
        assert!(rom_bytes(archive).unwrap_err().contains("no .nes"));
        assert!(rom_bytes(b"PK\x03\x04junk".to_vec()).is_err());
    }
}
