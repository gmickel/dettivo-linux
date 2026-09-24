#include "history_player.h"
#include "wav_metadata.h"

#include <QAudioOutput>
#include <QFile>
#include <QTimer>
#include <QUrl>
#include <QtEndian>

#include <algorithm>
#include <cmath>

namespace dettivo {

HistoryPlayer::HistoryPlayer(QObject *parent) : QObject(parent)
{
}

void HistoryPlayer::setSource(const QString &path)
{
    if (path == m_source)
        return;
    m_pendingSeek = -1;
    m_playRequested = false;
    if (m_player != nullptr) {
        m_player->stop();
        m_player->setSource(QUrl());
    }
    m_source = path;
    m_peaks.clear();
    m_sampleRate = 0;
    m_duration = 0;
    m_error.clear();
    setPosition(0);
    setPlaying(false);
    if (!path.isEmpty() && readWav(path, &m_peaks, &m_sampleRate, &m_duration, &m_error) && m_player != nullptr) {
        m_player->setSource(QUrl::fromLocalFile(path));
    }
    emit sourceChanged();
    // The clock and the fraction read the duration too.
    emit positionChanged();
}

void HistoryPlayer::applySample(const QVariantList &peaks, qint64 durationMs, qint64 positionMs)
{
    m_pendingSeek = -1;
    m_playRequested = false;
    if (m_player != nullptr) {
        m_player->stop();
        m_player->setSource(QUrl());
    }
    m_source = QStringLiteral("sample");
    m_peaks = peaks;
    m_sampleRate = 16000;
    m_duration = durationMs;
    m_error.clear();
    setPlaying(false);
    emit sourceChanged();
    setPosition(positionMs);
    emit positionChanged();
}

void HistoryPlayer::toggle()
{
    if (m_playing || m_playRequested)
        pause();
    else
        play();
}

void HistoryPlayer::play()
{
    if (!ready() || m_source == QStringLiteral("sample"))
        return;
    const qint64 startAt = m_position >= m_duration ? 0 : m_position;
    m_pendingSeek = startAt;
    m_playRequested = true;
    ensureOutput();
    setPosition(startAt);
    QTimer::singleShot(0, this, &HistoryPlayer::applyPendingSeek);
}

void HistoryPlayer::ensureOutput()
{
    if (m_output != nullptr)
        return;
    m_player = new QMediaPlayer(this);
    connect(m_player, &QMediaPlayer::positionChanged, this, [this](qint64 ms) {
        if (m_pendingSeek < 0)
            setPosition(ms);
    });
    connect(m_player, &QMediaPlayer::seekableChanged, this, [this](bool) {
        QTimer::singleShot(0, this, &HistoryPlayer::applyPendingSeek);
    });
    connect(m_player, &QMediaPlayer::playingChanged, this, [this](bool playing) { setPlaying(playing); });
    connect(m_player, &QMediaPlayer::mediaStatusChanged, this, [this](QMediaPlayer::MediaStatus status) {
        if (status == QMediaPlayer::LoadedMedia || status == QMediaPlayer::BufferedMedia)
            QTimer::singleShot(0, this, &HistoryPlayer::applyPendingSeek);
        if (status == QMediaPlayer::EndOfMedia) {
            setPlaying(false);
            setPosition(m_duration);
        }
    });
    connect(m_player, &QMediaPlayer::errorOccurred, this, [this](QMediaPlayer::Error, const QString &message) {
        m_pendingSeek = -1;
        m_playRequested = false;
        m_error = message;
        setPlaying(false);
        emit sourceChanged();
    });
    m_output = new QAudioOutput(this);
    m_player->setAudioOutput(m_output);
    m_player->setSource(QUrl::fromLocalFile(m_source));
}

void HistoryPlayer::pause()
{
    m_playRequested = false;
    if (m_player != nullptr)
        m_player->pause();
}

void HistoryPlayer::applyPendingSeek()
{
    if (m_player == nullptr || !m_player->isSeekable())
        return;
    const auto status = m_player->mediaStatus();
    if (status != QMediaPlayer::LoadedMedia && status != QMediaPlayer::BufferedMedia && status != QMediaPlayer::EndOfMedia)
        return;
    if (m_pendingSeek >= 0) {
        const qint64 position = m_pendingSeek;
        m_pendingSeek = -1;
        m_player->setPosition(position);
        setPosition(position);
    }
    if (m_playRequested) {
        m_playRequested = false;
        m_player->play();
    }
}

void HistoryPlayer::seek(double fraction)
{
    if (!ready())
        return;
    const qint64 ms = qint64(std::clamp(fraction, 0.0, 1.0) * double(m_duration));
    if (m_player != nullptr && m_source != QStringLiteral("sample")) {
        m_pendingSeek = ms;
        applyPendingSeek();
    }
    setPosition(ms);
}

void HistoryPlayer::setPosition(qint64 ms)
{
    ms = std::clamp<qint64>(ms, 0, std::max<qint64>(m_duration, 0));
    if (ms == m_position)
        return;
    m_position = ms;
    emit positionChanged();
}

void HistoryPlayer::setPlaying(bool playing)
{
    if (playing == m_playing)
        return;
    m_playing = playing;
    emit playingChanged();
}

bool HistoryPlayer::readWav(const QString &path, QVariantList *peaks, int *sampleRate, qint64 *durationMs, QString *error)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) {
        *error = file.errorString();
        return false;
    }
    WavMetadata metadata;
    if (!readWavMetadata(file, &metadata, error))
        return false;
    if (metadata.bits != 16) {
        *error = QStringLiteral("not 16-bit PCM audio");
        return false;
    }
    if (!file.seek(metadata.dataOffset)) {
        *error = file.errorString();
        return false;
    }
    const QByteArray bytes = file.read(metadata.dataBytes);
    if (bytes.size() != metadata.dataBytes) {
        *error = QStringLiteral("truncated WAV samples");
        return false;
    }
    const int channels = metadata.channels;
    const qsizetype frames = bytes.size() / (channels * 2);
    *sampleRate = metadata.sampleRate;
    *durationMs = metadata.durationMs;
    peaks->clear();
    const auto *samples = reinterpret_cast<const qint16 *>(bytes.constData());
    for (int bar = 0; bar < kBars; ++bar) {
        const qsizetype from = frames * bar / kBars;
        const qsizetype to = std::max(from + 1, frames * (bar + 1) / kBars);
        int peak = 0;
        for (qsizetype frame = from; frame < to && frame < frames; ++frame) {
            for (int c = 0; c < channels; ++c)
                peak = std::max(peak, std::abs(int(qFromLittleEndian<qint16>(&samples[frame * channels + c]))));
        }
        peaks->append(double(peak) / 32768.0);
    }
    return true;
}

QString HistoryPlayer::clockLabel(qint64 positionMs, qint64 durationMs)
{
    auto clock = [](qint64 ms) {
        const qint64 seconds = std::max<qint64>(0, (ms + 500) / 1000);
        return QStringLiteral("%1:%2").arg(seconds / 60).arg(seconds % 60, 2, 10, QLatin1Char('0'));
    };
    return QStringLiteral("%1 / %2").arg(clock(positionMs), clock(durationMs));
}

}  // namespace dettivo
