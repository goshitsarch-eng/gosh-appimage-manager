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
