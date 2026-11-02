// The Models step's language recommendation (ADR 0085): the language is
// `[dictation] language`, or the system locale's while that is `auto`;
// `speech.providers.list` with it answers the catalogue's ranking, whose
// first model is the recommended row. While no speech model is ready, no
// download runs and nothing was picked, that row is also the selected
// one. Nothing is written until the person picks, so an existing
// selection never changes.
#include "first_run_models.h"

#include <QJsonArray>
#include <QLocale>

#include <algorithm>

namespace dettivo {

QString FirstRunModels::languageFor(const QString &configured, const QString &locale)
{
    const QString code = configured.isEmpty() || configured == QStringLiteral("auto") ? locale : configured;
    return code.section(QLatin1Char('_'), 0, 0).section(QLatin1Char('-'), 0, 0).section(QLatin1Char('.'), 0, 0).toLower();
}

void FirstRunModels::requestRanking()
{
    m_link->call(QStringLiteral("config.get"), QJsonObject{{QStringLiteral("key"), QStringLiteral("dictation.language")}},
                 [this](const QJsonObject &result, const QJsonObject &error) {
                     if (!error.isEmpty() || m_link == nullptr)
                         return;
                     const QString configured =
                         result.value(QStringLiteral("entries")).toArray().first().toObject().value(QStringLiteral("value")).toString();
                     const QJsonObject params{{QStringLiteral("language"), languageFor(configured, QLocale::system().name())}};
                     m_link->call(QStringLiteral("speech.providers.list"), params, [this](const QJsonObject &answer, const QJsonObject &failure) {
                         if (failure.isEmpty())
                             applyRanking(answer.value(QStringLiteral("language_ranking")).toObject());
                     });
                 });
}

void FirstRunModels::applyRanking(const QJsonObject &ranking)
{
    const QJsonObject best = ranking.value(QStringLiteral("models")).toArray().first().toObject();
    const QString key = best.isEmpty() ? QString()
                                       : best.value(QStringLiteral("provider")).toString() + QLatin1Char('/') + best.value(QStringLiteral("model_id")).toString();
    if (key == m_recommendedKey)
        return;
    m_recommendedKey = key;
    m_recommendedLine = key.isEmpty() ? QString()
                                      : tr("Recommended for %1 · word error %2 %")
                                            .arg(QLocale::languageToString(QLocale(ranking.value(QStringLiteral("language")).toString()).language()))
                                            .arg(best.value(QStringLiteral("wer")).toDouble(), 0, 'f', 1);
    applyStatus(m_status);
}

void FirstRunModels::markRecommended()
{
    if (m_recommendedKey.isEmpty())
        return;
    const bool settled = m_picked || std::any_of(m_rows.cbegin(), m_rows.cend(), [](const Row &r) { return r.ready || r.downloading; });
    for (Row &r : m_rows) {
        const bool hit = r.provider + QLatin1Char('/') + r.id == m_recommendedKey;
        r.recommended = hit;
        if (hit)
            r.detail = m_recommendedLine;
        if (!settled)
            r.selected = hit;
    }
}

}  // namespace dettivo
