// Gosh AppImage Manager — ELF + AppImage magic parser (ports ElfParser).
// Pure function over bytes plus a bounded file reader. Never executes anything.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::limits;
use crate::types::{AppImageType, Architecture};

#[derive(Debug, Clone, Default)]
pub struct ElfInfo {
    pub valid: bool,
    pub truncated: bool,
    pub little_endian: bool,
    pub elf_class: u8,
    pub architecture: Architecture,
    pub app_image_type: AppImageType,
    pub payload_offset: i64,
    pub upd_info: Vec<u8>,
    pub error: String,
    pub dwarfs_magic: bool,
    pub squashfs_magic: bool,
}

fn u16_at(data: &[u8], off: usize, le: bool) -> Option<u16> {
    let b: [u8; 2] = data.get(off..off + 2)?.try_into().ok()?;
    Some(if le {
        u16::from_le_bytes(b)
    } else {
        u16::from_be_bytes(b)
    })
}

fn u32_at(data: &[u8], off: usize, le: bool) -> Option<u32> {
    let b: [u8; 4] = data.get(off..off + 4)?.try_into().ok()?;
    Some(if le {
        u32::from_le_bytes(b)
    } else {
        u32::from_be_bytes(b)
    })
}

fn u64_at(data: &[u8], off: usize, le: bool) -> Option<u64> {
    let b: [u8; 8] = data.get(off..off + 8)?.try_into().ok()?;
    Some(if le {
        u64::from_le_bytes(b)
    } else {
        u64::from_be_bytes(b)
    })
}

pub fn host_architecture() -> Architecture {
    match std::env::consts::ARCH {
        "x86_64" => Architecture::X86_64,
        "aarch64" => Architecture::AArch64,
        "x86" => Architecture::I386,
        "arm" => Architecture::Arm,
        _ => Architecture::Unknown,
    }
}

pub fn architecture_supported(arch: Architecture) -> bool {
    matches!(arch, Architecture::X86_64 | Architecture::AArch64)
}

pub fn parse(data: &[u8]) -> ElfInfo {
    let mut info = ElfInfo {
        little_endian: true,
        payload_offset: -1,
        ..Default::default()
    };
    if data.len() < 16 {
        info.truncated = true;
        info.error = "File too small to be an ELF AppImage".to_string();
        return info;
    }
    if data.len() < 4 || data[0] != 0x7f || data[1] != b'E' || data[2] != b'L' || data[3] != b'F' {
        info.error = "Not an ELF file".to_string();
        return info;
    }
    info.elf_class = data[4];
    let enc = data[5];
    info.little_endian = enc == 1;
    if enc != 1 && enc != 2 {
        info.error = "Unknown ELF endianness".to_string();
        return info;
    }
    // AppImage magic lives at ELF offset 8: 'A' 'I' <type>.
    if data.len() >= 11 && data[8] == b'A' && data[9] == b'I' {
        match data[10] {
            1 => {
                info.app_image_type = AppImageType::Type1;
                info.valid = true;
            }
            2 => {
                info.app_image_type = AppImageType::Type2;
                info.valid = true;
            }
            _ => {}
        }
    }
    let is64 = info.elf_class == 2;
    if info.elf_class != 1 && info.elf_class != 2 {
        info.error = "Unknown ELF class".to_string();
        return info;
    }
    let le = info.little_endian;
    // e_machine at offset 18 (endian-aware).
    if let Some(machine) = u16_at(data, 18, le) {
        info.architecture = match machine {
            0x3E => Architecture::X86_64,
            0xB7 => Architecture::AArch64,
            0x03 => Architecture::I386,
            0x28 => Architecture::Arm,
            _ => Architecture::Unknown,
        };
    }
    // Walk program + section headers with caps, tracking the payload offset.
    let mut payload: u64 = if is64 { 64 } else { 52 };
    let read_field = |off: usize, size: usize| -> Option<u64> {
        match (is64, size) {
            (_, 2) => u16_at(data, off, le).map(|v| v as u64),
            (_, 4) => u32_at(data, off, le).map(|v| v as u64),
            (true, 8) => u64_at(data, off, le),
            _ => None,
        }
    };
    let (phoff, phentsize, phnum, shoff, shentsize, shnum) = if is64 {
        if data.len() < 64 {
            info.truncated = true;
            info.error = "Truncated ELF header".to_string();
            return finish(info, payload);
        }
        (
            u64_at(data, 32, le).unwrap_or(0),
            u16_at(data, 54, le).unwrap_or(0) as u64,
            u16_at(data, 56, le).unwrap_or(0) as u64,
            u64_at(data, 40, le).unwrap_or(0),
            u16_at(data, 58, le).unwrap_or(0) as u64,
            u16_at(data, 60, le).unwrap_or(0) as u64,
        )
    } else {
        if data.len() < 52 {
            info.truncated = true;
            info.error = "Truncated ELF header".to_string();
            return finish(info, payload);
        }
        let _ = read_field;
        (
            u32_at(data, 28, le).unwrap_or(0) as u64,
            u16_at(data, 42, le).unwrap_or(0) as u64,
            u16_at(data, 44, le).unwrap_or(0) as u64,
            u32_at(data, 32, le).unwrap_or(0) as u64,
            u16_at(data, 46, le).unwrap_or(0) as u64,
            u16_at(data, 48, le).unwrap_or(0) as u64,
        )
    };
    // Program headers.
    let max_ph = phnum.min(128);
    for i in 0..max_ph {
        let base = phoff.saturating_add(i.saturating_mul(phentsize.max(1)));
        let (off_off, filesz_off) = if is64 { (8, 32) } else { (4, 16) };
        let (Some(p_offset), Some(p_filesz)) = (
            seg_val(data, base, off_off, is64, le),
            seg_val(data, base, filesz_off, is64, le),
        ) else {
            info.truncated = true;
            continue;
        };
        payload = payload.max(p_offset.saturating_add(p_filesz));
    }
    if phnum > max_ph {
        info.truncated = true;
    }
    // Section headers.
    let max_sh = shnum.min(256);
    for i in 0..max_sh {
        let base = shoff.saturating_add(i.saturating_mul(shentsize.max(1)));
        let (off_off, size_off) = if is64 { (24, 32) } else { (16, 20) };
        let (Some(s_offset), Some(s_size)) = (
            seg_val(data, base, off_off, is64, le),
            seg_val(data, base, size_off, is64, le),
        ) else {
            info.truncated = true;
            continue;
        };
        // .upd_info: small string section (name check needs shstrtab; instead
        // accept any section whose content looks like an update string when
        // it is in the expected size range).
        if s_size > 0 && s_size < limits::MAX_UPD_INFO_BYTES as u64 {
            if let Some(bytes) = read_section_bytes(data, s_offset, s_size) {
                if looks_like_upd_info(&bytes) {
                    let mut trimmed = bytes;
                    if let Some(nul) = trimmed.iter().position(|b| *b == 0) {
                        trimmed.truncate(nul);
                    }
                    if info.upd_info.is_empty() {
                        info.upd_info = trimmed;
                    }
                }
            }
        }
        payload = payload.max(s_offset.saturating_add(s_size));
        payload = payload.max(shoff.saturating_add(shnum.saturating_mul(shentsize.max(1))));
    }
    if shnum > max_sh {
        info.truncated = true;
    }
    finish(info, payload)
}

fn seg_val(data: &[u8], base: u64, field: u64, is64: bool, le: bool) -> Option<u64> {
    let off = base.checked_add(field)? as usize;
    if is64 {
        u64_at(data, off, le)
    } else {
        u32_at(data, off, le).map(|v| v as u64)
    }
}

fn read_section_bytes(data: &[u8], offset: u64, size: u64) -> Option<Vec<u8>> {
    let off = usize::try_from(offset).ok()?;
    let len = usize::try_from(size).ok()?;
    data.get(off..off.checked_add(len)?).map(|s| s.to_vec())
}

fn looks_like_upd_info(bytes: &[u8]) -> bool {
    // Embedded update strings look like "gh-releases-zsync|...|..." or
    // "zsync|https://...".
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let head: String = text.chars().take(64).collect();
    head.starts_with("gh-releases-zsync|")
        || head.starts_with("zsync|")
        || head.starts_with("bintray-zsync|")
}

/// Final validity gate: magic must name a known type and arch must be known.
fn finish(mut info: ElfInfo, payload: u64) -> ElfInfo {
    info.payload_offset = payload.min(i64::MAX as u64) as i64;
    let magic_ok = !matches!(info.app_image_type, AppImageType::Unknown);
    let arch_ok = !matches!(info.architecture, Architecture::Unknown);
    if magic_ok && arch_ok {
        info.error.clear();
        info.valid = true;
    } else {
        info.valid = false;
        if info.error.is_empty() {
            info.error = "Missing AppImage magic at ELF offset 8".to_string();
        }
    }
    info
}

/// Bounded file reader: reads at most `max_header_bytes`, then sniffs the
/// payload offset for squashfs/DwarFS magic.
pub fn parse_file(path: &Path, max_header_bytes: u64) -> ElfInfo {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => {
            return ElfInfo {
                error: "Cannot read ELF header".to_string(),
                ..Default::default()
            };
        }
    };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let to_read = size.min(max_header_bytes).min(16 * 1024 * 1024);
    let mut data = vec![0u8; to_read as usize];
    let mut filled = 0;
    while filled < data.len() {
        match file.read(&mut data[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(_) => {
                return ElfInfo {
                    error: "Cannot read ELF header".to_string(),
                    ..Default::default()
                };
            }
        }
    }
    data.truncate(filled);
    let mut info = parse(&data);
    if size < 64 {
        info.truncated = true;
    }
    if info.payload_offset > 0
        && (info.payload_offset as u64) < size
        && file
            .seek(SeekFrom::Start(info.payload_offset as u64))
            .is_ok()
    {
        let mut magic = [0u8; 8];
        let mut got = 0;
        while got < magic.len() {
            match file.read(&mut magic[got..]) {
                Ok(0) => break,
                Ok(n) => got += n,
                Err(_) => break,
            }
        }
        if magic.starts_with(b"hsqs") || magic.starts_with(b"sqsh") {
            info.squashfs_magic = true;
        }
        if magic.starts_with(b"DWARFS") {
            info.dwarfs_magic = true;
            if info.app_image_type == AppImageType::Type2 {
                info.app_image_type = AppImageType::Dwarfs;
            }
        }
    }
    info
}
