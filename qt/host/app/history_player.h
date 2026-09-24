// Playback of a retained take (fn-22 R1, ADR 0025): QMediaPlayer over
// the item's `microphone.wav`, the position for the played portion of
// the audio strip, and the bars of the strip read straight from the WAV
// samples (16 kHz mono PCM by the store's contract), so the strip draws
// the take before a frame of it has played. The media backend and audio
// output are created on first play; waveform inspection does not need
// FFmpeg initialization or a connection to the sound server.
#pragma once

#include <QMediaPlayer>
#include <QObject>
#include <QString>
#include <QVariantList>

class QAudioOutput;

namespace dettivo {

class HistoryPlayer : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString source READ source WRITE setSource NOTIFY sourceChanged)
    Q_PROPERTY(bool ready READ ready NOTIFY sourceChanged)
    Q_PROPERTY(QVariantList peaks READ peaks NOTIFY sourceChanged)
    Q_PROPERTY(int sampleRate READ sampleRate NOTIFY sourceChanged)
    Q_PROPERTY(qint64 duration READ duration NOTIFY sourceChanged)
    Q_PROPERTY(qint64 position READ position NOTIFY positionChanged)
    Q_PROPERTY(double fraction READ fraction NOTIFY positionChanged)
    Q_PROPERTY(QString clock READ clock NOTIFY positionChanged)
    Q_PROPERTY(bool playing READ playing NOTIFY playingChanged)
    Q_PROPERTY(QString error READ error NOTIFY sourceChanged)

public:
    /// Bars the strip draws.
    static constexpr int kBars = 48;

    explicit HistoryPlayer(QObject *parent = nullptr);

    QString source() const { return m_source; }
    /// Points the player at a WAV file; empty stops and clears it.
    void setSource(const QString &path);
    /// True when the file parsed as PCM audio.
    bool ready() const { return !m_peaks.isEmpty(); }
    /// `kBars` values in 0..1, the peak of each slice of the take.
    QVariantList peaks() const { return m_peaks; }
    int sampleRate() const { return m_sampleRate; }
    /// Milliseconds, from the WAV header.
    qint64 duration() const { return m_duration; }
    qint64 position() const { return m_position; }
    /// The played portion in 0..1.
    double fraction() const { return m_duration > 0 ? double(m_position) / double(m_duration) : 0.0; }
    /// `0:01 / 0:04`.
    QString clock() const { return clockLabel(m_position, m_duration); }
    bool playing() const { return m_playing; }
    /// Why the file could not be read or played, empty otherwise.
    QString error() const { return m_error; }

    Q_INVOKABLE void toggle();
    Q_INVOKABLE void play();
    Q_INVOKABLE void pause();
    /// Seeks to a fraction of the take.
    Q_INVOKABLE void seek(double fraction);

    /// Shows a take without a file (the sample render): its bars, length
    /// and played portion.
    void applySample(const QVariantList &peaks, qint64 durationMs, qint64 positionMs);
    /// Reads the peaks, the rate and the duration of a WAV file; `error`
    /// names why it could not be read.
    static bool readWav(const QString &path, QVariantList *peaks, int *sampleRate, qint64 *durationMs, QString *error);
    /// The strip's position label: `0:01 / 0:04`.
    static QString clockLabel(qint64 positionMs, qint64 durationMs);

signals:
    void sourceChanged();
    void positionChanged();
    void playingChanged();

private:
    void setPosition(qint64 ms);
    void setPlaying(bool playing);
    /// Creates the decoder and audio output on first playback.
    void ensureOutput();
    /// Applies the preview position after loading and starts if requested.
    void applyPendingSeek();

    QMediaPlayer *m_player = nullptr;
    QAudioOutput *m_output = nullptr;
    QString m_source, m_error;
    QVariantList m_peaks;
    int m_sampleRate = 0;
    qint64 m_duration = 0;
    qint64 m_position = 0;
    qint64 m_pendingSeek = -1;
    bool m_playRequested = false;
    bool m_playing = false;
};

}  // namespace dettivo
