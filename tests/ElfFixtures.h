#pragma once

#include "core/Types.h"

#include <QByteArray>
#include <QDir>
#include <QFile>
#include <QtEndian>

namespace GoshAim {
namespace TestFixt {

inline void writeU16(QByteArray &data, int offset, quint16 value)
{
    uchar buf[2];
    qToLittleEndian(value, buf);
    data[offset] = char(buf[0]);
    data[offset + 1] = char(buf[1]);
}

inline void writeU32(QByteArray &data, int offset, quint32 value)
{
    uchar buf[4];
    qToLittleEndian(value, buf);
    for (int i = 0; i < 4; ++i) {
        data[offset + i] = char(buf[i]);
    }
}

inline void writeU64(QByteArray &data, int offset, quint64 value)
{
    uchar buf[8];
    qToLittleEndian(value, buf);
    for (int i = 0; i < 8; ++i) {
        data[offset + i] = char(buf[i]);
    }
}

inline QByteArray makeElf64(Architecture arch, quint8 appImageType, const QByteArray &updInfo = {}, const QByteArray &payload = {})
{
    const QByteArray shstr = QByteArray("\0.upd_info\0.shstrtab\0", 22);
    const int ehsize = 64;
    const int phsize = 56;
    const int shsize = 64;
    const int phoff = 64;
    const int payloadOff = ehsize + phsize;
    const int updOff = payloadOff + payload.size();
    const int strOff = updOff + updInfo.size();
    const int shoff = strOff + shstr.size();
    const int total = shoff + shsize * 3;
    QByteArray data(total, '\0');
    data[0] = 0x7f;
    data[1] = 'E';
    data[2] = 'L';
    data[3] = 'F';
    data[4] = 2;
    data[5] = 1;
    data[6] = 1;
    data[8] = 'A';
    data[9] = 'I';
    data[10] = char(appImageType);
    writeU16(data, 16, 3);
    quint16 machine = 0x3E;
    if (arch == Architecture::AArch64) {
        machine = 0xB7;
    } else if (arch == Architecture::I386) {
        machine = 0x03;
    } else if (arch == Architecture::Arm) {
        machine = 0x28;
    }
    writeU16(data, 18, machine);
    writeU32(data, 20, 1);
    writeU64(data, 32, phoff);
    writeU64(data, 40, shoff);
    writeU16(data, 52, ehsize);
    writeU16(data, 54, phsize);
    writeU16(data, 56, 1);
    writeU16(data, 58, shsize);
    writeU16(data, 60, 3);
    writeU16(data, 62, 2);
    writeU32(data, phoff, 1);
    writeU32(data, phoff + 4, 5);
    writeU64(data, phoff + 8, 0);
    writeU64(data, phoff + 32, quint64(payloadOff));
    writeU64(data, phoff + 40, quint64(payloadOff));
    if (!payload.isEmpty()) {
        data.replace(payloadOff, payload.size(), payload);
    }
    if (!updInfo.isEmpty()) {
        data.replace(updOff, updInfo.size(), updInfo);
    }
    data.replace(strOff, shstr.size(), shstr);
    writeU32(data, shoff + shsize, 1);
    writeU32(data, shoff + shsize + 4, 1);
    writeU64(data, shoff + shsize + 24, quint64(updOff));
    writeU64(data, shoff + shsize + 32, quint64(updInfo.size()));
    writeU32(data, shoff + shsize * 2, 11);
    writeU32(data, shoff + shsize * 2 + 4, 3);
    writeU64(data, shoff + shsize * 2 + 24, quint64(strOff));
    writeU64(data, shoff + shsize * 2 + 32, quint64(shstr.size()));
    return data;
}

inline QString writeFile(const QString &dir, const QString &name, const QByteArray &data)
{
    QDir().mkpath(dir);
    const QString path = dir + QLatin1Char('/') + name;
    QFile file(path);
    if (!file.open(QIODevice::WriteOnly)) {
        return {};
    }
    file.write(data);
    file.close();
    return path;
}

} // namespace TestFixt
} // namespace GoshAim
