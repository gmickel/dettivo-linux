#include "wav_metadata.h"

#include <limits>
#include <QtEndian>

namespace dettivo {

bool readWavMetadata(QFile &file, WavMetadata *metadata, QString *error)
{
    const auto reject = [error]() {
        *error = QStringLiteral("invalid or truncated PCM WAV");
        return false;
    };
    if (!file.seek(0))
        return reject();
    const QByteArray header = file.read(12);
    if (header.size() != 12 || !header.startsWith("RIFF") || header.mid(8, 4) != "WAVE")
        return reject();
    const qint64 end = qint64(qFromLittleEndian<quint32>(header.constData() + 4)) + 8;
    if (end < 12 || end > file.size())
        return reject();
    WavMetadata found;
    bool hasFormat = false;
    qint64 at = 12;
    // Metadata inspection on the GUI thread has a fixed read budget;
    // large unknown payloads are skipped without reading their contents.
    for (int chunks = 0; chunks < 8192 && end - at >= 8; ++chunks) {
        if (!file.seek(at))
            return reject();
        const QByteArray chunk = file.read(8);
        if (chunk.size() != 8)
            return reject();
        const qint64 size = qFromLittleEndian<quint32>(chunk.constData() + 4);
        const qint64 body = at + 8;
        if (size > end - body || size + (size % 2) > end - body)
            return reject();
        if (chunk.startsWith("fmt ")) {
            if (size < 16)
                return reject();
            const QByteArray format = file.read(16);
            if (format.size() != 16 || qFromLittleEndian<quint16>(format.constData()) != 1)
                return reject();
            found.channels = qFromLittleEndian<quint16>(format.constData() + 2);
            const quint32 rate = qFromLittleEndian<quint32>(format.constData() + 4);
            const quint32 byteRate = qFromLittleEndian<quint32>(format.constData() + 8);
            const quint16 alignment = qFromLittleEndian<quint16>(format.constData() + 12);
            found.bits = qFromLittleEndian<quint16>(format.constData() + 14);
            if (found.channels == 0 || rate == 0 || rate > quint32(std::numeric_limits<int>::max())
                || found.bits == 0 || found.bits % 8 != 0
                || alignment != found.channels * (found.bits / 8)
                || qint64(byteRate) != qint64(rate) * alignment)
                return reject();
            found.sampleRate = int(rate);
            hasFormat = true;
        } else if (chunk.startsWith("data")) {
            if (!hasFormat)
                return reject();
            const qint64 alignment = found.channels * (found.bits / 8);
            if (size % alignment != 0)
                return reject();
            found.dataOffset = body;
            found.dataBytes = size;
            found.durationMs = (size / alignment) * 1000 / found.sampleRate;
            *metadata = found;
            return true;
        }
        at = body + size + (size % 2);
    }
    return reject();
}

}  // namespace dettivo
