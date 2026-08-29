#include "ElfParser.h"

#include <QFile>
#include <QSysInfo>
#include <QtEndian>

#include <cstring>

namespace GoshAim {

namespace {

quint16 u16(const QByteArray &data, int offset, bool le)
{
    if (offset + 2 > data.size()) {
        return 0;
    }
    const auto *p = reinterpret_cast<const uchar *>(data.constData() + offset);
    return le ? qFromLittleEndian<quint16>(p) : qFromBigEndian<quint16>(p);
}

quint32 u32(const QByteArray &data, int offset, bool le)
{
    if (offset + 4 > data.size()) {
        return 0;
    }
    const auto *p = reinterpret_cast<const uchar *>(data.constData() + offset);
    return le ? qFromLittleEndian<quint32>(p) : qFromBigEndian<quint32>(p);
}

quint64 u64(const QByteArray &data, int offset, bool le)
{
    if (offset + 8 > data.size()) {
        return 0;
    }
    const auto *p = reinterpret_cast<const uchar *>(data.constData() + offset);
    return le ? qFromLittleEndian<quint64>(p) : qFromBigEndian<quint64>(p);
}

Architecture machineToArch(quint16 machine)
{
    switch (machine) {
    case 0x3E:
        return Architecture::X86_64;
    case 0xB7:
        return Architecture::AArch64;
    case 0x03:
        return Architecture::I386;
    case 0x28:
        return Architecture::Arm;
    default:
        return Architecture::Unknown;
    }
}

QByteArray readSectionName(const QByteArray &data, quint64 offset, quint32 size)
{
    if (offset >= static_cast<quint64>(data.size())) {
        return {};
    }
    const quint64 end = qMin(offset + size, static_cast<quint64>(data.size()));
    QByteArray slice = data.mid(static_cast<int>(offset), static_cast<int>(end - offset));
    const int nul = slice.indexOf('\0');
    if (nul >= 0) {
        slice = slice.left(nul);
    }
    return slice;
}

} // namespace

ElfInfo ElfParser::parseFile(const QString &path, qint64 maxHeaderBytes)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) {
        ElfInfo info;
        info.error = QStringLiteral("Cannot read ELF header");
        return info;
    }
    const qint64 toRead = qMin(file.size(), maxHeaderBytes);
    const QByteArray data = file.read(toRead);
    ElfInfo info = parse(data);
    if (file.size() < 64) {
        info.truncated = true;
    }
    if (info.payloadOffset > 0 && info.payloadOffset < file.size()) {
        file.seek(info.payloadOffset);
        const QByteArray magic = file.read(8);
        if (magic.startsWith("hsqs") || magic.startsWith("sqsh")) {
            info.squashfsMagic = true;
        }
        if (magic.startsWith("DWARFS")) {
            info.dwarfsMagic = true;
            if (info.appImageType == AppImageType::Type2) {
                info.appImageType = AppImageType::Dwarfs;
            }
        }
    }
    return info;
}

ElfInfo ElfParser::parse(const QByteArray &data)
{
    ElfInfo info;
    if (data.size() < 16) {
        info.truncated = true;
        info.error = QStringLiteral("File too small to be an ELF AppImage");
        return info;
    }
    if (!(data.size() >= 4 && static_cast<unsigned char>(data[0]) == 0x7f && data[1] == 'E' && data[2] == 'L'
          && data[3] == 'F')) {
        info.error = QStringLiteral("Not an ELF file");
        return info;
    }
    info.elfClass = static_cast<unsigned char>(data[4]);
    const int dataEnc = static_cast<unsigned char>(data[5]);
    info.littleEndian = dataEnc == 1;
    if (dataEnc != 1 && dataEnc != 2) {
        info.error = QStringLiteral("Unknown ELF endianness");
        return info;
    }
    if (data.size() >= 11 && data[8] == 'A' && data[9] == 'I') {
        const unsigned char typeByte = static_cast<unsigned char>(data[10]);
        if (typeByte == 1) {
            info.appImageType = AppImageType::Type1;
            info.valid = true;
        } else if (typeByte == 2) {
            info.appImageType = AppImageType::Type2;
            info.valid = true;
        }
    }
    if (data.size() < 64) {
        info.truncated = true;
        info.error = QStringLiteral("Truncated ELF header");
        return info;
    }
    const bool le = info.littleEndian;
    const bool is64 = info.elfClass == 2;
    if (info.elfClass != 1 && info.elfClass != 2) {
        info.error = QStringLiteral("Unknown ELF class");
        return info;
    }
    const quint16 machine = u16(data, 18, le);
    info.architecture = machineToArch(machine);

    quint64 phoff = 0;
    quint64 shoff = 0;
    quint16 phentsize = 0;
    quint16 phnum = 0;
    quint16 shentsize = 0;
    quint16 shnum = 0;
    quint16 shstrndx = 0;
    if (is64) {
        phoff = u64(data, 32, le);
        shoff = u64(data, 40, le);
        phentsize = u16(data, 54, le);
        phnum = u16(data, 56, le);
        shentsize = u16(data, 58, le);
        shnum = u16(data, 60, le);
        shstrndx = u16(data, 62, le);
    } else {
        phoff = u32(data, 28, le);
        shoff = u32(data, 32, le);
        phentsize = u16(data, 42, le);
        phnum = u16(data, 44, le);
        shentsize = u16(data, 46, le);
        shnum = u16(data, 48, le);
        shstrndx = u16(data, 50, le);
    }

    qint64 end = is64 ? 64 : 52;
    const int maxPh = qMin(int(phnum), 128);
    for (int i = 0; i < maxPh; ++i) {
        const quint64 off = phoff + static_cast<quint64>(i) * phentsize;
        if (off + phentsize > static_cast<quint64>(data.size())) {
            info.truncated = true;
            break;
        }
        quint64 filesz = 0;
        quint64 poff = 0;
        if (is64) {
            poff = u64(data, static_cast<int>(off + 8), le);
            filesz = u64(data, static_cast<int>(off + 32), le);
        } else {
            poff = u32(data, static_cast<int>(off + 4), le);
            filesz = u32(data, static_cast<int>(off + 16), le);
        }
        end = qMax(end, static_cast<qint64>(poff + filesz));
    }

    if (shoff > 0 && shnum > 0 && shentsize > 0 && shstrndx < shnum) {
        const int maxSh = qMin(int(shnum), 256);
        const quint64 strOff = shoff + static_cast<quint64>(shstrndx) * shentsize;
        quint64 strtabOff = 0;
        quint64 strtabSize = 0;
        if (strOff + shentsize <= static_cast<quint64>(data.size())) {
            if (is64) {
                strtabOff = u64(data, static_cast<int>(strOff + 24), le);
                strtabSize = u64(data, static_cast<int>(strOff + 32), le);
            } else {
                strtabOff = u32(data, static_cast<int>(strOff + 16), le);
                strtabSize = u32(data, static_cast<int>(strOff + 20), le);
            }
        }
        for (int i = 0; i < maxSh; ++i) {
            const quint64 off = shoff + static_cast<quint64>(i) * shentsize;
            if (off + shentsize > static_cast<quint64>(data.size())) {
                info.truncated = true;
                break;
            }
            quint32 nameOff = 0;
            quint64 soff = 0;
            quint64 ssize = 0;
            if (is64) {
                nameOff = u32(data, static_cast<int>(off), le);
                soff = u64(data, static_cast<int>(off + 24), le);
                ssize = u64(data, static_cast<int>(off + 32), le);
            } else {
                nameOff = u32(data, static_cast<int>(off), le);
                soff = u32(data, static_cast<int>(off + 16), le);
                ssize = u32(data, static_cast<int>(off + 20), le);
            }
            end = qMax(end, static_cast<qint64>(soff + ssize));
            if (strtabOff > 0) {
                const QByteArray name = readSectionName(data, strtabOff + nameOff, 32);
                if (name == QByteArray(".upd_info") && ssize > 0 && ssize < 4096) {
                    info.updInfo = readSectionName(data, soff, static_cast<quint32>(qMin(ssize, quint64(4096))));
                }
            }
        }
        end = qMax(end, static_cast<qint64>(shoff + static_cast<quint64>(shnum) * shentsize));
    }

    info.payloadOffset = end;
    if (info.appImageType != AppImageType::Unknown && info.architecture != Architecture::Unknown) {
        info.valid = true;
        info.error.clear();
    } else if (info.appImageType == AppImageType::Unknown) {
        if (info.error.isEmpty()) {
            info.error = QStringLiteral("Missing AppImage magic at ELF offset 8");
        }
    }
    return info;
}

Architecture ElfParser::hostArchitecture()
{
    const QString arch = QSysInfo::currentCpuArchitecture();
    if (arch == QLatin1String("x86_64") || arch == QLatin1String("amd64")) {
        return Architecture::X86_64;
    }
    if (arch == QLatin1String("arm64") || arch == QLatin1String("aarch64")) {
        return Architecture::AArch64;
    }
    if (arch == QLatin1String("i386") || arch == QLatin1String("i686")) {
        return Architecture::I386;
    }
    if (arch.startsWith(QLatin1String("arm"))) {
        return Architecture::Arm;
    }
    return Architecture::Unknown;
}

bool ElfParser::architectureSupported(Architecture architecture)
{
    return architecture == Architecture::X86_64 || architecture == Architecture::AArch64;
}

} // namespace GoshAim
