#include "first_run_models.h"
#include "status_format.h"

#include <QJsonValue>
#include <QLoggingCategory>
#include <QProcessEnvironment>

#include <algorithm>

namespace dettivo {

Q_LOGGING_CATEGORY(lcFirstRunModels, "dettivo.app.firstrun.models")

namespace {

QString megabytes(quint64 bytes)
{
    return QString::number(qRound(double(bytes) / 1e6));
}

QString languagesLine(const QJsonArray &languages, const QString &provider)
{
    QStringList parts;
    for (const QJsonValue &l : languages) {
        const QString lang = l.toString();
        if (lang == QStringLiteral("multilingual"))
            parts.append(provider == QStringLiteral("parakeet") ? QObject::tr("25 European languages") : QObject::tr("99 languages"));
        else if (lang == QStringLiteral("en"))
            parts.append(QObject::tr("English"));
        else
            parts.append(lang);
    }
    return parts.join(QStringLiteral(" · "));
}

}  // namespace

FirstRunModels::FirstRunModels(DaemonLink *link, QObject *parent) : QAbstractListModel(parent), m_link(link)
{
    if (m_link != nullptr)
        connect(m_link, &DaemonLink::notification, this, &FirstRunModels::handleNotification);
}

int FirstRunModels::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : int(m_rows.size());
}

QHash<int, QByteArray> FirstRunModels::roleNames() const
{
    return {{KeyRole, "key"},           {ProviderRole, "provider"}, {IdRole, "modelId"},
            {NameRole, "name"},         {DetailRole, "detail"},     {SizeRole, "size"},
            {StateRole, "state"},       {ProgressRole, "progress"}, {RecommendedRole, "recommended"},
            {SelectedRole, "selected"}, {ReadyRole, "ready"},       {ErrorRole, "error"}};
}

QVariant FirstRunModels::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_rows.size())
        return {};
    const Row &r = m_rows.at(index.row());
    switch (role) {
    case KeyRole:
        return r.provider + QLatin1Char('/') + r.id;
    case ProviderRole:
        return r.provider;
    case IdRole:
        return r.id;
    case NameRole:
        return r.name;
    case DetailRole:
        return r.detail;
    case SizeRole:
        return r.size;
    case StateRole:
        return r.state;
    case ProgressRole:
        return r.progress;
    case RecommendedRole:
        return r.recommended;
    case SelectedRole:
        return r.selected;
    case ReadyRole:
        return r.ready;
    case ErrorRole:
        return r.error;
    default:
        return {};
    }
}

QString FirstRunModels::defaultFor(const QString &gpu)
{
    return gpu == QStringLiteral("vulkan") ? QStringLiteral("parakeet/parakeet-v3") : QStringLiteral("whisper/small.en");
}

void FirstRunModels::setTier(const QString &gpu)
{
    if (m_gpu == gpu)
        return;
    m_gpu = gpu;
    emit summaryChanged();
}

QString FirstRunModels::tierLine() const
{
    return m_gpu == QStringLiteral("vulkan") ? tr("Vulkan will run them.") : tr("The CPU will run them.");
}

bool FirstRunModels::ready() const
{
    return std::any_of(m_rows.cbegin(), m_rows.cend(), [](const Row &r) { return r.selected && r.ready; });
}

QString FirstRunModels::selectedKey() const
{
    for (const Row &r : m_rows) {
        if (r.selected)
            return r.provider + QLatin1Char('/') + r.id;
    }
    return {};
}

bool FirstRunModels::downloading() const
{
    return std::any_of(m_rows.cbegin(), m_rows.cend(), [](const Row &r) { return r.downloading; });
}

double FirstRunModels::downloadFraction() const
{
    for (const Row &r : m_rows) {
        if (r.downloading)
            return r.progress;
    }
    return 0.0;
}

QString FirstRunModels::downloadFile() const
{
    for (const Row &r : m_rows) {
        if (r.downloading)
            return r.quantization.isEmpty() ? r.id : r.id + QStringLiteral(" · ") + r.quantization;
    }
    return {};
}

QString FirstRunModels::downloadLine() const
{
    for (const Row &r : m_rows) {
        if (!r.downloading)
            continue;
        QStringList parts{tr("%1 of %2 MB").arg(megabytes(r.bytesDone), megabytes(r.bytesTotal))};
        if (m_bytesPerSecond > 0)
            parts.append(tr("%1 MB/s").arg(QString::number(m_bytesPerSecond / 1e6, 'f', 0)));
        parts.append(tr("sha256 pending"));
        return parts.join(QStringLiteral(" · "));
    }
    return {};
}

int FirstRunModels::rowOf(const QString &provider, const QString &id) const
{
    for (int i = 0; i < m_rows.size(); ++i) {
        if (m_rows[i].provider == provider && m_rows[i].id == id)
            return i;
    }
    return -1;
}

void FirstRunModels::updateRow(int row)
{
    if (row < 0 || row >= m_rows.size())
        return;
    emit dataChanged(index(row), index(row));
    emit summaryChanged();
}

void FirstRunModels::applyStatus(const QJsonObject &result)
{
    const QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    m_modelsDir = format::homePath(result.value(QStringLiteral("models_dir")).toString(), env);
    QList<Row> rows;
    QString selectedKey;
    for (const QJsonValue &v : result.value(QStringLiteral("models")).toArray()) {
        const QJsonObject m = v.toObject();
        const QString readiness = m.value(QStringLiteral("readiness")).toString();
        const bool ready = readiness == QStringLiteral("ready") || readiness == QStringLiteral("unverified");
        const QJsonValue recommended = m.value(QStringLiteral("recommended_for"));
        const bool selected = m.value(QStringLiteral("is_selected")).toBool(false);
        if (m.value(QStringLiteral("kind")).toString() == QStringLiteral("vad"))
            continue;
        if (recommended.isNull() && !ready && !selected)
            continue;
        if (!m.value(QStringLiteral("available")).toBool(true))
            continue;
        Row r;
        r.provider = m.value(QStringLiteral("provider")).toString();
        r.id = m.value(QStringLiteral("id")).toString();
        r.name = (r.provider == QStringLiteral("whisper") ? tr("Whisper %1") : tr("Parakeet %1"))
                     .arg(r.provider == QStringLiteral("whisper") ? r.id : m.value(QStringLiteral("display_name")).toString().remove(QStringLiteral("Parakeet ")));
        const QString languages = languagesLine(m.value(QStringLiteral("languages")).toArray(), r.provider);
        r.detail = recommended.isString() ? recommended.toString() : languages;
        r.recommended = recommended.isString() && m.value(QStringLiteral("is_default")).toBool(false) && r.provider == QStringLiteral("parakeet");
        r.size = tr("%1 MB").arg(megabytes(quint64(m.value(QStringLiteral("size_bytes")).toDouble())));
        r.selected = selected;
        r.ready = ready;
        r.bytesDone = quint64(m.value(QStringLiteral("bytes_done")).toDouble());
        r.bytesTotal = quint64(m.value(QStringLiteral("bytes_total")).toDouble());
        r.downloading = readiness == QStringLiteral("downloading");
        r.progress = r.bytesTotal > 0 ? double(r.bytesDone) / double(r.bytesTotal) : 0.0;
        r.error = m.value(QStringLiteral("error")).toString();
        if (r.downloading)
            r.state = tr("%1 %").arg(qRound(r.progress * 100));
        else if (ready)
            r.state = tr("ready");
        else if (readiness == QStringLiteral("quarantined"))
            r.state = tr("failed");
        else if (!r.error.isEmpty())
            r.state = tr("failed");
        else if (readiness == QStringLiteral("partial"))
            r.state = tr("resume");
        else
            r.state = tr("get");
        if (r.id.contains(QStringLiteral("q8")) || r.provider == QStringLiteral("parakeet"))
            r.quantization = QStringLiteral("q8_0");
        if (selected)
            selectedKey = r.provider + QLatin1Char('/') + r.id;
        rows.append(r);
    }
    beginResetModel();
    m_rows = rows;
    endResetModel();
    emit countChanged();
    emit summaryChanged();
}

// The capabilities can arrive after the providers: the local row is
// recomputed from the last providers answer, so it never stays on
// "soon" once the daemon has declared the download.
void FirstRunModels::setLlmMethods(const QStringList &methods)
{
    m_llmMethods = methods;
    applyProviders(m_providers, m_llmMethods);
}

void FirstRunModels::refresh(bool includeSpeech)
{
    if (m_link == nullptr || !m_link->connected())
        return;
    if (includeSpeech) {
        m_link->call(QStringLiteral("speech.models.status"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
            if (error.isEmpty())
                applyStatus(result);
        });
    }
    m_link->call(QStringLiteral("llm.providers.list"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty())
            applyProviders(result.value(QStringLiteral("providers")).toArray(), m_llmMethods);
    });
    m_link->call(QStringLiteral("config.get"), QJsonObject{{QStringLiteral("key"), QStringLiteral("llm.provider")}},
                 [this](const QJsonObject &result, const QJsonObject &error) {
                     if (error.isEmpty())
                         applyLlmProvider(result.value(QStringLiteral("entries")).toArray().first().toObject().value(QStringLiteral("value")).toString());
                 });
}

void FirstRunModels::applyProviders(const QJsonArray &providers, const QStringList &llmMethods)
{
    // The local engine's catalogue download is `llm.models.download`; a
    // daemon that does not declare it keeps the local row on "soon" with
    // the provider's own detail, and the tick writes `[llm]` alone.
    m_providers = providers;
    m_localDownloadOffered = llmMethods.contains(QStringLiteral("llm.models.download"));
    m_localTitle = tr("Qwen3 4B Instruct, local");
    m_localSize = QStringLiteral("2.5 GB");
    m_localLine = tr("Rewrites dictation in Enhanced mode and writes meeting analysis. Runs in its own process, unloads when idle.");
    m_localState = m_localDownloadOffered ? tr("get") : tr("soon");
    m_ollamaAvailable = false;
    m_ollamaLine = tr("Not running on localhost:11434. Polish still works without any model.");
    for (const QJsonValue &v : providers) {
        const QJsonObject p = v.toObject();
        const QString id = p.value(QStringLiteral("id")).toString();
        const bool available = p.value(QStringLiteral("available")).toBool(false);
        const QString detail = p.value(QStringLiteral("detail")).toString();
        if (id == QStringLiteral("local")) {
            if (available)
                m_localState = tr("ready");
            else if (!m_localDownloadOffered && !detail.isEmpty())
                m_localLine = m_localLine + QLatin1Char(' ') + detail.at(0).toUpper() + detail.mid(1) + QLatin1Char('.');
        } else if (id == QStringLiteral("ollama")) {
            m_ollamaAvailable = available;
            m_ollamaLine = available ? tr("Running, %1 answers. Polish still works without any model.").arg(p.value(QStringLiteral("model")).toString())
                                     : tr("Not running on localhost:11434. Polish still works without any model.");
        }
    }
    emit enhancedChanged();
}

void FirstRunModels::applyLlmProvider(const QString &provider)
{
    const QString choice = provider == QStringLiteral("local") || provider == QStringLiteral("ollama") ? provider : QStringLiteral("none");
    if (choice == m_enhanced)
        return;
    m_enhanced = choice;
    emit enhancedChanged();
}

void FirstRunModels::writeSelection(const QString &provider, const QString &id)
{
    if (m_link == nullptr || !m_link->connected())
        return;
    const QJsonObject params{{QStringLiteral("provider"), provider}, {QStringLiteral("model"), id}};
    m_link->call(QStringLiteral("speech.selection.set"), params, [this](const QJsonObject &, const QJsonObject &error) {
        if (!error.isEmpty())
            qCWarning(lcFirstRunModels) << "speech.selection.set refused:" << error.value(QStringLiteral("message")).toString();
        refresh();
    });
}

void FirstRunModels::select(const QString &provider, const QString &id)
{
    const int row = rowOf(provider, id);
    if (row < 0)
        return;
    for (int i = 0; i < m_rows.size(); ++i)
        m_rows[i].selected = i == row;
    emit dataChanged(index(0), index(int(m_rows.size()) - 1));
    emit summaryChanged();
    writeSelection(provider, id);
    if (!m_rows[row].ready && !m_rows[row].downloading)
        download(provider, id);
}

void FirstRunModels::download(const QString &provider, const QString &id)
{
    if (m_link == nullptr || !m_link->connected())
        return;
    const QJsonObject params{{QStringLiteral("provider"), provider}, {QStringLiteral("model"), id}};
    m_rate.restart();
    m_rateBytes = 0;
    m_bytesPerSecond = 0.0;
    m_link->call(QStringLiteral("speech.models.download"), params, [this, provider, id](const QJsonObject &, const QJsonObject &error) {
        if (!error.isEmpty()) {
            const int row = rowOf(provider, id);
            if (row >= 0) {
                m_rows[row].error = error.value(QStringLiteral("message")).toString();
                m_rows[row].state = tr("failed");
                updateRow(row);
            }
            return;
        }
        refresh();
    });
}

void FirstRunModels::cancel(const QString &provider, const QString &id)
{
    if (m_link == nullptr || !m_link->connected())
        return;
    const QJsonObject params{{QStringLiteral("provider"), provider}, {QStringLiteral("model"), id}};
    m_link->call(QStringLiteral("speech.models.cancel"), params, [this](const QJsonObject &, const QJsonObject &) { refresh(); });
}

void FirstRunModels::setEnhanced(const QString &choice)
{
    const QString provider = choice == QStringLiteral("local") || choice == QStringLiteral("ollama") ? choice : QStringLiteral("auto");
    applyLlmProvider(choice);
    if (m_link == nullptr || !m_link->connected())
        return;
    const QJsonObject params{{QStringLiteral("key"), QStringLiteral("llm.provider")}, {QStringLiteral("value"), provider}};
    m_link->call(QStringLiteral("config.set"), params, [](const QJsonObject &, const QJsonObject &error) {
        if (!error.isEmpty())
            qCWarning(lcFirstRunModels) << "config.set llm.provider refused:" << error.value(QStringLiteral("message")).toString();
    });
    if (choice == QStringLiteral("local") && m_localDownloadOffered)
        downloadLocalModel();
}

void FirstRunModels::downloadLocalModel()
{
    // `llm.models.status` names the selected catalogue model and whether
    // it is on disk; the download asks for that id, as Settings does.
    m_link->call(QStringLiteral("llm.models.status"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty()) {
            m_localState = tr("failed");
            m_localLine = error.value(QStringLiteral("message")).toString();
            emit enhancedChanged();
            return;
        }
        m_localModel = result.value(QStringLiteral("selected")).toString();
        QString readiness;
        for (const QJsonValue &v : result.value(QStringLiteral("models")).toArray()) {
            const QJsonObject m = v.toObject();
            if (m.value(QStringLiteral("id")).toString() == m_localModel)
                readiness = m.value(QStringLiteral("readiness")).toString();
        }
        if (m_localModel.isEmpty()) {
            m_localState = tr("failed");
            m_localLine = tr("[llm] model names no catalogue model.");
            emit enhancedChanged();
            return;
        }
        if (readiness == QStringLiteral("ready") || readiness == QStringLiteral("unverified") || readiness == QStringLiteral("downloading")) {
            m_localState = readiness == QStringLiteral("downloading") ? tr("0 %") : tr("ready");
            emit enhancedChanged();
            return;
        }
        const QJsonObject params{{QStringLiteral("model"), m_localModel}};
        m_link->call(QStringLiteral("llm.models.download"), params, [this](const QJsonObject &, const QJsonObject &refusal) {
            if (!refusal.isEmpty()) {
                // The refusal stays on the row until the next attempt.
                m_localState = tr("failed");
                m_localLine = refusal.value(QStringLiteral("message")).toString();
                emit enhancedChanged();
                return;
            }
            m_localState = tr("0 %");
            emit enhancedChanged();
        });
    });
}

void FirstRunModels::applyLocalDownload(const QJsonObject &payload)
{
    const QString state = payload.value(QStringLiteral("state")).toString();
    const double done = payload.value(QStringLiteral("bytes_done")).toDouble();
    const double total = payload.value(QStringLiteral("bytes_total")).toDouble();
    if (state == QStringLiteral("running")) {
        m_localState = tr("%1 %").arg(total > 0 ? qRound(done / total * 100) : 0);
    } else if (state == QStringLiteral("done")) {
        m_localState = tr("ready");
    } else if (state == QStringLiteral("cancelled")) {
        m_localState = tr("get");
    } else {
        m_localState = tr("failed");
        m_localLine = payload.value(QStringLiteral("error")).toString(m_localLine);
    }
    emit enhancedChanged();
}

void FirstRunModels::handleNotification(const QString &topic, const QJsonObject &payload)
{
    if (topic != QStringLiteral("model.download"))
        return;
    if (payload.value(QStringLiteral("provider")).toString() == QStringLiteral("llm")
        && payload.value(QStringLiteral("model")).toString() == m_localModel) {
        applyLocalDownload(payload);
        return;
    }
    const int row = rowOf(payload.value(QStringLiteral("provider")).toString(), payload.value(QStringLiteral("model")).toString());
    if (row < 0)
        return;
    Row &r = m_rows[row];
    const QString state = payload.value(QStringLiteral("state")).toString();
    r.bytesDone = quint64(payload.value(QStringLiteral("bytes_done")).toDouble());
    r.bytesTotal = quint64(payload.value(QStringLiteral("bytes_total")).toDouble());
    r.progress = r.bytesTotal > 0 ? double(r.bytesDone) / double(r.bytesTotal) : 0.0;
    if (state == QStringLiteral("running")) {
        r.downloading = true;
        r.state = tr("%1 %").arg(qRound(r.progress * 100));
        if (!m_rate.isValid())
            m_rate.start();
        const qint64 ms = m_rate.elapsed();
        if (ms >= 500 && r.bytesDone > m_rateBytes) {
            m_bytesPerSecond = double(r.bytesDone - m_rateBytes) * 1000.0 / double(ms);
            m_rateBytes = r.bytesDone;
            m_rate.restart();
        } else if (m_rateBytes == 0) {
            m_rateBytes = r.bytesDone;
        }
        updateRow(row);
        return;
    }
    r.downloading = false;
    m_bytesPerSecond = 0.0;
    r.error = payload.value(QStringLiteral("error")).toString();
    if (state == QStringLiteral("done")) {
        r.ready = true;
        r.state = tr("ready");
    } else if (state == QStringLiteral("cancelled")) {
        r.state = tr("resume");
    } else {
        r.state = tr("failed");
    }
    updateRow(row);
    // The daemon's row is the truth for readiness after a verification.
    refresh();
}

}  // namespace dettivo
