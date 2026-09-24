#include "models_table.h"


namespace dettivo {

int ModelsTable::downloadCount(const QJsonValue &models)
{
    if (!models.isArray())
        return -1;
    int count = 0;
    const QStringList readinessValues{QStringLiteral("ready"), QStringLiteral("unverified"),
        QStringLiteral("downloading"), QStringLiteral("partial"), QStringLiteral("missing"), QStringLiteral("quarantined")};
    for (const auto &value : models.toArray()) {
        if (!value.isObject())
            return -1;
        const auto readiness = value.toObject().value(QStringLiteral("readiness"));
        if (!readiness.isString() || !readinessValues.contains(readiness.toString()))
            return -1;
        count += readiness.toString() == QStringLiteral("downloading") ? 1 : 0;
    }
    return count;
}

int ModelsTable::activeDownloads() const
{
    if (m_speechDownloads < 0 || m_llmDownloads < 0)
        return -1;
    return m_speechDownloads + m_llmDownloads;
}

void ModelsTable::ensureDownloadStatus()
{
    if (m_speechDownloads < 0 || m_llmDownloads < 0)
        refreshCatalogue();
}

void ModelsTable::invalidateDownloads()
{
    m_speechDownloads = m_llmDownloads = -1;
    m_cataloguePending = 0;
    ++m_catalogueGeneration;
}

void ModelsTable::refreshCatalogue()
{
    if (m_link == nullptr || !m_link->connected() || m_cataloguePending != 0)
        return;
    m_speechDownloads = m_llmDownloads = -1;
    m_cataloguePending = 2;
    const auto generation = m_catalogueGeneration;
    m_link->call(QStringLiteral("speech.models.status"), {}, [this, generation](const QJsonObject &result, const QJsonObject &error) {
        if (generation != m_catalogueGeneration)
            return;
        if (error.isEmpty() && result.value(QStringLiteral("models")).isArray())
            applySpeech(result);
        --m_cataloguePending;
    });
    m_link->call(QStringLiteral("llm.models.status"), {}, [this, generation](const QJsonObject &result, const QJsonObject &error) {
        if (generation != m_catalogueGeneration)
            return;
        if (error.isEmpty() && result.value(QStringLiteral("models")).isArray())
            applyLlm(result);
        --m_cataloguePending;
    });
}

}  // namespace dettivo
