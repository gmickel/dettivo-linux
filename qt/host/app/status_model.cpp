#include "status_model.h"
#include "config_binding.h"
#include "status_format.h"

#include <QDateTime>
#include <QJsonArray>
#include <QLoggingCategory>

#include <cmath>

namespace dettivo {

Q_LOGGING_CATEGORY(lcStatus, "dettivo.app.status")

StatusModel::StatusModel(DaemonLink *link, ConfigBinding *config, QObject *parent)
    : QObject(parent), m_link(link), m_config(config)
{
    m_lastCall = tr("not tracked");
    m_levels.reserve(kLevelSamples);
    for (int i = 0; i < kLevelSamples; ++i)
        m_levels.append(0.0);
    m_clockTimer.setInterval(15000);
    connect(&m_clockTimer, &QTimer::timeout, this, &StatusModel::tickClock);
    m_awayTimer.setSingleShot(true);
    m_awayTimer.setInterval(kAwayGraceMs);
    connect(&m_awayTimer, &QTimer::timeout, this, [this]() {
        if (!daemonConnected())
            setDaemonState(QStringLiteral("away"));
    });
    if (m_link != nullptr) {
        connect(m_link, &DaemonLink::notification, this, &StatusModel::handleNotification);
        connect(m_link, &DaemonLink::connectedChanged, this, &StatusModel::onConnected);
        connect(m_link, &DaemonLink::overflow, this, [this](qint64) { refresh(); });
    }
    if (m_config != nullptr)
        connect(m_config, &ConfigBinding::changed, this, &StatusModel::readConfig);
    tickClock();
}

QStringList StatusModel::topics()
{
    return {QStringLiteral("dictation.state"), QStringLiteral("audio.level"),     QStringLiteral("engine.state"),
            QStringLiteral("job.progress"),    QStringLiteral("model.download"), QStringLiteral("config.changed"),
            QStringLiteral("meeting.state"),   QStringLiteral("meeting.segment")};
}

void StatusModel::start()
{
    m_clockTimer.start();
    m_awayTimer.start();
    if (m_link != nullptr) {
        m_link->subscribe(topics());
        if (m_link->connected())
            onConnected(true);
    }
}

void StatusModel::setDaemonState(const QString &state)
{
    if (m_daemonState == state)
        return;
    m_daemonState = state;
    emit connectionChanged();
}

void StatusModel::onConnected(bool connected)
{
    if (connected) {
        m_awayTimer.stop();
        setDaemonState(QStringLiteral("connected"));
        refresh();
    } else {
        m_dictationState = QStringLiteral("idle");
        emit dictationChanged();
        m_awayTimer.start();
        emit connectionChanged();
    }
}

void StatusModel::refresh()
{
    if (!daemonConnected())
        return;
    readHealth();
    readCapabilities();
    readTarget();
    readDevices();
    readDictation();
    if (m_config != nullptr)
        m_config->refresh();
}

void StatusModel::readHealth()
{
    m_link->call(QStringLiteral("system.health"), {}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty())
            return;
        m_healthOk = result.value(QStringLiteral("ok")).toBool(true);
        const QString recording = result.value(QStringLiteral("recording_state")).toString();
        if (recording == QStringLiteral("idle") && m_dictationState != QStringLiteral("idle")) {
            m_dictationState = QStringLiteral("idle");
            emit dictationChanged();
        }
        emit factsChanged();
    });
    m_link->call(QStringLiteral("system.version"), {}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty()) {
            m_version = result.value(QStringLiteral("api_version")).toString();
            emit factsChanged();
        }
    });
}

void StatusModel::readCapabilities()
{
    m_link->call(QStringLiteral("system.capabilities"), {}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty())
            return;
        const QJsonObject auth = result.value(QStringLiteral("auth")).toObject();
        m_socketMode = auth.value(QStringLiteral("ipc_mode")).toString();
        const QJsonObject platform = result.value(QStringLiteral("platform")).toObject();
        m_gpu = platform.value(QStringLiteral("gpu")).toString();
        m_compositor = platform.value(QStringLiteral("compositor")).toString();
        const QJsonObject rest = result.value(QStringLiteral("rest")).toObject();
        m_restState = rest.value(QStringLiteral("enabled")).toBool(false)
                          ? tr("on · %1").arg(rest.value(QStringLiteral("port")).toInt())
                          : tr("off");
        emit factsChanged();
    });
}

void StatusModel::readTarget()
{
    m_link->call(QStringLiteral("insert.target"), {}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty())
            return;
        const QJsonObject target = result.value(QStringLiteral("target")).toObject();
        const bool self = target.value(QStringLiteral("is_dettivo")).toBool(false);
        m_targetApp = self ? QString() : format::appName(target.value(QStringLiteral("app_id")).toString());
        emit factsChanged();
    });
}

void StatusModel::setMcpHosts(const QStringList &hosts)
{
    const QString text = hosts.isEmpty() ? tr("none configured") : hosts.join(QStringLiteral(" · "));
    if (text != m_mcpHosts) {
        m_mcpHosts = text;
        emit factsChanged();
    }
}

void StatusModel::readDevices()
{
    m_link->call(QStringLiteral("audio.devices"), {}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty())
            return;
        const QString pinned = result.value(QStringLiteral("pinned")).toString();
        const QString wanted = pinned.isEmpty() ? result.value(QStringLiteral("default_source")).toString() : pinned;
        QString name;
        const QJsonArray devices = result.value(QStringLiteral("devices")).toArray();
        for (const QJsonValue &d : devices) {
            const QJsonObject device = d.toObject();
            if (device.value(QStringLiteral("name")).toString() == wanted) {
                name = device.value(QStringLiteral("description")).toString();
                break;
            }
        }
        if (name.isEmpty())
            name = result.value(QStringLiteral("pipewire")).toBool(false) ? tr("no input") : tr("no PipeWire");
        m_inputName = name;
        emit factsChanged();
    });
}

void StatusModel::readDictation()
{
    m_link->call(QStringLiteral("dictation.status"), {}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty())
            return;
        const bool active = result.value(QStringLiteral("is_active")).toBool(false);
        const QString message = result.value(QStringLiteral("job")).toObject().value(QStringLiteral("message")).toString();
        QString state = QStringLiteral("idle");
        if (active)
            state = message.startsWith(QStringLiteral("listening")) ? QStringLiteral("recording") : QStringLiteral("transcribing");
        if (state != m_dictationState) {
            m_dictationState = state;
            emit dictationChanged();
        }
    });
}

void StatusModel::readConfig()
{
    if (m_config == nullptr)
        return;
    m_holdChord = format::chord(m_config->text(QStringLiteral("hotkeys.hold")));
    m_toggleChord = format::chord(m_config->text(QStringLiteral("hotkeys.toggle")));
    emit factsChanged();
}

void StatusModel::handleNotification(const QString &topic, const QJsonObject &payload)
{
    if (topic == QStringLiteral("config.changed")) {
        readHealth();
        readCapabilities();
        return;
    }
    if (topic == QStringLiteral("dictation.state")) {
        QString state = payload.value(QStringLiteral("state")).toString();
        QString error = m_dictationError;
        if (state == QStringLiteral("failed"))
            error = payload.value(QStringLiteral("reason")).toString(tr("Dictation failed. Check the selected model and microphone."));
        else if (state == QStringLiteral("recording") || state == QStringLiteral("cancelled"))
            error.clear();
        if (state == QStringLiteral("cancelled") || state == QStringLiteral("failed"))
            state = QStringLiteral("idle");
        if (state != m_dictationState || error != m_dictationError) {
            m_dictationState = state;
            m_dictationError = error;
            emit dictationChanged();
        }
        if (state == QStringLiteral("idle")) {
            for (int i = 0; i < m_levels.size(); ++i)
                m_levels[i] = 0.0;
            emit levelsChanged();
        }
        return;
    }
    if (topic == QStringLiteral("audio.level")) {
        const double rms = payload.value(QStringLiteral("rms")).toDouble();
        const double peak = payload.value(QStringLiteral("peak")).toDouble();
        m_levels.removeFirst();
        m_levels.append(qBound(0.0, rms * 3.0 + peak * 0.2, 1.0));
        emit levelsChanged();
    }
}

void StatusModel::setDictationError(const QString &message)
{
    if (message == m_dictationError)
        return;
    m_dictationError = message;
    emit dictationChanged();
}

void StatusModel::startDictation()
{
    if (!daemonConnected())
        return;
    setDictationError(QString());
    // No mode and no language on the request: the daemon runs the
    // session on `[dictation] mode` and `[dictation] language`, the
    // values the strip shows, as the hotkey path does.
    m_link->call(QStringLiteral("dictation.start"), QJsonObject{},
                 [this](const QJsonObject &, const QJsonObject &error) {
                     if (!error.isEmpty()) {
                         setDictationError(error.value(QStringLiteral("message")).toString(tr("Could not start dictation.")));
                         qCWarning(lcStatus) << "dictation.start refused:" << error.value(QStringLiteral("message")).toString();
                     }
                 });
}

void StatusModel::stopDictation()
{
    if (!daemonConnected())
        return;
    m_link->call(QStringLiteral("dictation.stop"), {}, [this](const QJsonObject &, const QJsonObject &error) {
        if (!error.isEmpty()) {
            setDictationError(error.value(QStringLiteral("message")).toString(tr("Could not stop dictation.")));
            qCWarning(lcStatus) << "dictation.stop refused:" << error.value(QStringLiteral("message")).toString();
        }
    });
}

void StatusModel::toggleDictation()
{
    if (m_dictationState == QStringLiteral("recording"))
        stopDictation();
    else if (m_dictationState == QStringLiteral("idle"))
        startDictation();
}

void StatusModel::tickClock()
{
    if (m_clockFrozen)
        return;
    const QString now = format::clock(QDateTime::currentDateTime());
    if (now == m_clock)
        return;
    m_clock = now;
    emit clockChanged();
}

void StatusModel::setClockForTesting(const QString &text)
{
    m_clockFrozen = true;
    m_clock = text;
    emit clockChanged();
}

void StatusModel::applySample(const QJsonObject &sample)
{
    m_awayTimer.stop();
    m_daemonState = QStringLiteral("connected");
    m_healthOk = true;
    m_dictationState = QStringLiteral("idle");
    m_holdChord = sample.value(QStringLiteral("hold")).toString();
    m_toggleChord = sample.value(QStringLiteral("toggle")).toString();
    m_targetApp = sample.value(QStringLiteral("target")).toString();
    m_socketMode = sample.value(QStringLiteral("socket_mode")).toString();
    m_gpu = sample.value(QStringLiteral("gpu")).toString();
    m_compositor = sample.value(QStringLiteral("compositor")).toString();
    m_inputName = sample.value(QStringLiteral("input")).toString();
    m_restState = sample.value(QStringLiteral("rest")).toString();
    m_mcpHosts = sample.value(QStringLiteral("mcp")).toString();
    m_lastCall = sample.value(QStringLiteral("last_call")).toString();
    setClockForTesting(sample.value(QStringLiteral("clock")).toString());
    if (sample.value(QStringLiteral("levels")).toBool(false)) {
        // A take in progress: bars that rise and fall like speech.
        for (int i = 0; i < m_levels.size(); ++i)
            m_levels[i] = 0.2 + 0.7 * std::fabs(std::sin(double(i) * 0.9)) * (0.6 + 0.4 * std::fabs(std::cos(double(i) * 0.23)));
        emit levelsChanged();
    }
    emit connectionChanged();
    emit factsChanged();
    emit dictationChanged();
}

}  // namespace dettivo
