#pragma once

#include "Types.h"

#include <QByteArray>
#include <QString>

namespace GoshAim {

struct ElfInfo {
    bool valid = false;
    bool truncated = false;
    bool littleEndian = true;
    int elfClass = 0;
    Architecture architecture = Architecture::Unknown;
    AppImageType appImageType = AppImageType::Unknown;
    qint64 payloadOffset = -1;
    QByteArray updInfo;
    QString error;
    bool dwarfsMagic = false;
    bool squashfsMagic = false;
};

class ElfParser
{
public:
    static ElfInfo parseFile(const QString &path, qint64 maxHeaderBytes = 1024 * 1024);
    static ElfInfo parse(const QByteArray &data);
    static Architecture hostArchitecture();
    static bool architectureSupported(Architecture architecture);
};

} // namespace GoshAim
