mod common;

use goshaim_core::elf;
use goshaim_core::inspector::make_test_elf;
use goshaim_core::types::{AppImageType, Architecture};

#[test]
fn type2_x86_64_parses_clean() {
    let info = elf::parse(&make_test_elf(Architecture::X86_64, AppImageType::Type2));
    assert!(info.error.is_empty(), "error: {}", info.error);
    assert!(info.valid);
    assert_eq!(info.app_image_type, AppImageType::Type2);
    assert_eq!(info.architecture, Architecture::X86_64);
}

#[test]
fn type1_and_aarch64_parse() {
    let info = elf::parse(&make_test_elf(Architecture::AArch64, AppImageType::Type1));
    assert!(info.error.is_empty(), "error: {}", info.error);
    assert_eq!(info.app_image_type, AppImageType::Type1);
    assert_eq!(info.architecture, Architecture::AArch64);
}

#[test]
fn non_elf_rejected() {
    let info = elf::parse(b"definitely not an elf binary at all....");
    assert_eq!(info.error, "Not an ELF file");
    assert!(!info.valid);
}

#[test]
fn tiny_file_reports_truncation() {
    let info = elf::parse(b"\x7fELF");
    assert!(info.truncated);
    assert!(!info.error.is_empty());
}

#[test]
fn bad_class_rejected() {
    let mut data = make_test_elf(Architecture::X86_64, AppImageType::Type2);
    data[4] = 9;
    let info = elf::parse(&data);
    assert_eq!(info.error, "Unknown ELF class");
}

#[test]
fn bad_endian_rejected() {
    let mut data = make_test_elf(Architecture::X86_64, AppImageType::Type2);
    data[5] = 9;
    let info = elf::parse(&data);
    assert_eq!(info.error, "Unknown ELF endianness");
}

#[test]
fn missing_magic_reported() {
    let mut data = make_test_elf(Architecture::X86_64, AppImageType::Type2);
    data[8] = b'X';
    data[9] = b'X';
    let info = elf::parse(&data);
    assert!(!info.valid);
    assert_eq!(info.error, "Missing AppImage magic at ELF offset 8");
}

#[test]
fn unknown_type_byte_reported() {
    let data = make_test_elf(Architecture::X86_64, AppImageType::Unknown);
    let info = elf::parse(&data);
    assert!(!info.valid);
    assert!(!info.error.is_empty());
}

#[test]
fn only_x86_64_and_aarch64_supported() {
    assert!(elf::architecture_supported(Architecture::X86_64));
    assert!(elf::architecture_supported(Architecture::AArch64));
    assert!(!elf::architecture_supported(Architecture::I386));
    assert!(!elf::architecture_supported(Architecture::Arm));
    assert!(!elf::architecture_supported(Architecture::Unknown));
}

#[test]
fn unreadable_file_reports_cannot_read() {
    let info = elf::parse_file(
        std::path::Path::new("/nonexistent/goshaim-elf-probe.AppImage"),
        1024 * 1024,
    );
    assert_eq!(info.error, "Cannot read ELF header");
}

/// Section header table at the end of the runtime, as in a real Type-2 AppImage:
/// the squashfs payload starts exactly where the section table ends.
#[test]
fn payload_offset_is_the_end_of_the_section_table() {
    // 64-byte header, a string table at 64..80, then three section headers at 256.
    let file = common::elf_layout(256, &[(0, 0, 0), (3, 64, 16)], &[]);
    let info = elf::parse(&file);
    assert!(info.error.is_empty(), "error: {}", info.error);
    assert_eq!(info.payload_offset, 256 + 2 * 64);
}

/// A NOBITS section (.bss, .tbss) occupies no bytes in the file, but its header
/// still carries a size. Counting it pushed the payload offset past the real end
/// of the ELF, so unsquashfs could not find the squashfs superblock.
#[test]
fn payload_offset_ignores_nobits_sections() {
    let file = common::elf_layout(
        256,
        &[(0, 0, 0), (3, 64, 16), (8 /* SHT_NOBITS */, 80, 1 << 20)],
        &[],
    );
    let info = elf::parse(&file);
    assert_eq!(
        info.payload_offset,
        256 + 3 * 64,
        "a 1 MiB .bss must not move the payload"
    );
}

/// The program header table also bounds the ELF: a PT_LOAD that covers 4096 bytes
/// puts the payload at 4096 (the layout of the QA probe fixture).
#[test]
fn payload_offset_follows_a_loadable_segment() {
    let file = common::elf_layout(0, &[], &[(0, 4096)]);
    let info = elf::parse(&file);
    assert_eq!(info.payload_offset, 4096);
}

/// A runtime larger than the header window keeps its section table past it.
/// The offset must still be the real end of the tables, not what the window held.
#[test]
fn section_table_beyond_the_header_window_still_sets_the_payload() {
    let h = common::Harness::new();
    let shoff = 2 * 1024 * 1024; // beyond the 1 MiB header window
    let mut bytes = common::elf_layout(shoff, &[(0, 0, 0), (3, 64, 16)], &[]);
    let end_of_elf = bytes.len();
    bytes.extend_from_slice(common::LAYOUT_PAYLOAD);
    let path = h.tmp.path().join("Wide.AppImage");
    std::fs::write(&path, &bytes).unwrap();

    let info = elf::parse_file(&path, 1024 * 1024);
    assert_eq!(info.payload_offset, end_of_elf as i64);
    assert!(
        info.squashfs_magic,
        "the squashfs sits at the computed offset"
    );
}

/// Audit R6-01 (inspect path): a header whose program table sits at the top of the
/// address space is read as truncated. It must not overflow while it is read,
/// which panics in a debug build.
#[test]
fn a_program_header_table_at_the_top_of_the_address_space_is_read_without_panicking() {
    let mut data = vec![0u8; 64];
    data[..4].copy_from_slice(b"\x7fELF");
    data[4] = 2; // 64-bit
    data[5] = 1; // little-endian
    data[6] = 1; // version
    data[8] = b'A';
    data[9] = b'I';
    data[10] = 2; // Type 2
    data[18..20].copy_from_slice(&0x3Eu16.to_le_bytes()); // x86_64
    data[32..40].copy_from_slice(&(u64::MAX - 10).to_le_bytes()); // e_phoff
    data[54..56].copy_from_slice(&56u16.to_le_bytes()); // e_phentsize
    data[56..58].copy_from_slice(&1u16.to_le_bytes()); // e_phnum

    let info = goshaim_core::elf::parse(&data);
    assert!(
        info.truncated,
        "a table that cannot be read is reported as truncated"
    );
}
