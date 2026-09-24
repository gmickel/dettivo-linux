#include "first_run_model.h"
#include "first_run_models.h"

#include <QDateTime>
#include <QJsonArray>
#include <QLoggingCategory>

namespace dettivo {
Q_DECLARE_LOGGING_CATEGORY(lcFirstRun)

void FirstRunModel::onConnected(bool connected)
{
    if (!connected) {
        m_connectionProcessed = false;
        m_pressPoll.stop();
        return;
    }
    if (m_connectionProcessed)
        return;
    m_connectionProcessed = true;
    if (m_forcedOpen) {
        activate(false);
    } else if (!m_completed && !m_decided) {
        m_answers = 0;
        decide();
    } else if (active()) {
        activate(false);
    }
}

void FirstRunModel::activate(bool reuseEligibility)
{
    readKeys();
    readSnippet(!reuseEligibility);
    if (!reuseEligibility)
        readHotkeysStatus();
    readDevices();
    readSelectionLine();
    m_models->refresh(!reuseEligibility);
    m_link->call(QStringLiteral("system.capabilities"), {}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty()) {
            const QJsonObject platform = result.value(QStringLiteral("platform")).toObject();
            m_platformCompositor = platform.value(QStringLiteral("compositor")).toString();
            m_compositor = compositorLabel(m_platformCompositor);
            m_models->setTier(platform.value(QStringLiteral("gpu")).toString());
            QStringList methods;
            for (const QJsonValue &m : result.value(QStringLiteral("llm")).toObject().value(QStringLiteral("methods")).toArray())
                methods.append(m.toString());
            m_models->setLlmMethods(methods);
            emit keysChanged();
        }
    });
    if (m_step == QStringLiteral("keys"))
        m_pressPoll.start();
}

void FirstRunModel::decide()
{
    // Three answers decide: a speech model on disk, and either bound
    // actions on a daemon backend or a sourced compositor snippet.
    m_link->call(QStringLiteral("speech.models.status"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        bool ready = false;
        if (error.isEmpty()) {
            m_models->applyStatus(result);
            for (const QJsonValue &v : result.value(QStringLiteral("models")).toArray()) {
                const QJsonObject m = v.toObject();
                const QString readiness = m.value(QStringLiteral("readiness")).toString();
                if (m.value(QStringLiteral("kind")).toString() != QStringLiteral("vad")
                    && (readiness == QStringLiteral("ready") || readiness == QStringLiteral("unverified")))
                    ready = true;
            }
        }
        m_answers |= ready ? 0b1001 : 0b0001;
        settle();
    });
    m_link->call(QStringLiteral("hotkeys.status"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty())
            applyHotkeysStatus(result);
        const bool bound = error.isEmpty() && !result.value(QStringLiteral("bound")).toArray().isEmpty();
        m_answers |= bound ? 0b10010 : 0b00010;
        settle();
    });
    m_link->call(QStringLiteral("hotkeys.setup"), QJsonObject{{QStringLiteral("write"), false}}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty())
            applySetup(result);
        const bool sourced = error.isEmpty() && result.value(QStringLiteral("sourced")).toBool(false);
        m_answers |= sourced ? 0b100100 : 0b000100;
        settle();
    });
}

void FirstRunModel::settle()
{
    if ((m_answers & 0b111) != 0b111 || m_decided)
        return;
    const bool modelReady = (m_answers & 0b1000) != 0;
    const bool bound = (m_answers & 0b10000) != 0;
    const bool sourced = (m_answers & 0b100000) != 0;
    m_decided = true;
    m_required = !(modelReady && (bound || sourced));
    qCInfo(lcFirstRun) << "first run" << (m_required ? "required" : "not required") << "model" << modelReady << "bound" << bound << "sourced" << sourced;
    if (!m_required) {
        m_completed = true;
        emit completed(QDateTime::currentDateTimeUtc().toString(Qt::ISODate));
    }
    if (m_required)
        activate(true);
    emit decisionChanged();
}

}  // namespace dettivo
