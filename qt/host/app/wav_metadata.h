#pragma once

#include <QFile>

namespace dettivo {

struct WavMetadata {
    int channels = 0;
    int sampleRate = 0;
    int bits = 0;
    qint64 dataOffset = 0;
    qint64 dataBytes = 0;
    qint64 durationMs = -1;
};

bool readWavMetadata(QFile &file, WavMetadata *metadata, QString *error);

}  // namespace dettivo
