#include "models_table.h"
#include "status_format.h"

#include <QJsonValue>
#include <QDir>
#include <QFileInfo>
#include <QProcessEnvironment>
#include <QVariantMap>

#include <algorithm>

namespace dettivo {

namespace {

QString sizeText(quint64 bytes)
{
    if (bytes >= 1'000'000'000)
        return QObject::tr("%1 GB").arg(QString::number(double(bytes) / 1e9, 'f', 1));
    return QObject::tr("%1 MB").arg(qRound(double(bytes) / 1e6));
}

QString engineOf(const QString &provider)
{
    return provider == QStringLiteral("llm") ? QStringLiteral("llama.cpp")
                                             : (provider == QStringLiteral("parakeet") ? QStringLiteral("parakeet.cpp") : QStringLiteral("whisper.cpp"));
}

QString keyOf(const QString &binary)
{
    return binary.section(QLatin1Char('-'), -1);
}

}  // namespace

ModelsTable::ModelsTable(DaemonLink *link, QObject *parent) : QAbstractListModel(parent), m_link(link)
{
    if (m_link != nullptr) {
        connect(m_link, &DaemonLink::connectedChanged, this, &ModelsTable::onConnected);
        connect(m_link, &DaemonLink::notification, this, &ModelsTable::handleNotification);
    }
}

int ModelsTable::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : int(m_rows.size());
}

QHash<int, QByteArray> ModelsTable::roleNames() const
{
    return {{KeyRole, "key"},         {ProviderRole, "provider"},       {IdRole, "modelId"},       {NameRole, "name"},
            {DetailRole, "detail"},   {SizeRole, "size"},               {BackendRole, "backend"},  {StateRole, "state"},
            {ProgressRole, "progress"}, {ReadyRole, "ready"},           {DownloadingRole, "downloading"},
            {SelectedRole, "selected"}, {WarmRole, "warm"},             {ErrorRole, "error"}};
}

QVariant ModelsTable::data(const QModelIndex &index, int role) const
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
    case BackendRole:
        return r.backend;
    case StateRole:
        return r.state;
    case ProgressRole:
        return r.progress;
    case ReadyRole:
        return r.ready;
    case DownloadingRole:
        return r.downloading;
    case SelectedRole:
        return r.selected;
    case WarmRole:
        return r.warm;
    case ErrorRole:
        return r.error;
    default:
        return {};
    }
}

void ModelsTable::start()
{
    setActive(true);
}

void ModelsTable::setActive(bool active)
{
    if (m_active == active)
        return;
    m_active = active;
    if (m_link != nullptr && m_link->connected())
        onConnected(true);
}

void ModelsTable::onConnected(bool connected)
{
    if (!connected)
        invalidateDownloads();
    if (connected && m_active)
        refresh();
}

void ModelsTable::refresh()
{
    if (m_link == nullptr || !m_link->connected())
        return;
    refreshCatalogue();
    m_link->call(QStringLiteral("speech.engines"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty())
            applyEngines(result.value(QStringLiteral("engines")).toArray());
    });
    m_link->call(QStringLiteral("system.capabilities"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty())
            return;
        m_gpu = result.value(QStringLiteral("platform")).toObject().value(QStringLiteral("gpu")).toString();
        emit summaryChanged();
    });
}

ModelsTable::Row ModelsTable::rowFrom(const QJsonObject &m)
{
    Row r;
    r.source = m.value(QStringLiteral("source")).toString();
    r.provider = m.value(QStringLiteral("provider")).toString();
    r.id = m.value(QStringLiteral("id")).toString();
    r.path = m.value(QStringLiteral("path")).toString();
    r.name = m.value(QStringLiteral("display_name")).toString(r.id);
    if (r.provider == QStringLiteral("whisper") && !r.name.startsWith(QStringLiteral("Whisper")))
        r.name = QStringLiteral("Whisper ") + r.name;
    QStringList detail{engineOf(r.provider)};
    for (const QJsonValue &l : m.value(QStringLiteral("languages")).toArray()) {
        const QString lang = l.toString();
        if (lang == QStringLiteral("multilingual"))
            detail.append(r.provider == QStringLiteral("parakeet") ? tr("25 European languages") : tr("99 languages"));
        else if (lang == QStringLiteral("en"))
            detail.append(tr("English"));
        else
            detail.append(lang);
    }
    if (r.provider == QStringLiteral("llm"))
        detail.append(tr("Enhanced and meeting analysis"));
    const QString recommended = m.value(QStringLiteral("recommended_for")).toString();
    if (!recommended.isEmpty())
        detail.append(recommended);
    detail.append(m.value(QStringLiteral("license")).toString());
    detail.removeAll(QString());
    r.detail = detail.join(QStringLiteral(" · "));
    r.bytesDone = quint64(m.value(QStringLiteral("bytes_done")).toDouble());
    r.bytesTotal = quint64(m.value(QStringLiteral("bytes_total")).toDouble());
    r.size = sizeText(quint64(m.value(QStringLiteral("size_bytes")).toDouble()));
    const QString readiness = m.value(QStringLiteral("readiness")).toString();
    r.ready = readiness == QStringLiteral("ready") || readiness == QStringLiteral("unverified");
    r.downloading = readiness == QStringLiteral("downloading");
    r.progress = r.bytesTotal > 0 ? double(r.bytesDone) / double(r.bytesTotal) : 0.0;
    r.selected = m.value(QStringLiteral("is_selected")).toBool(false);
    r.error = m.value(QStringLiteral("error")).toString();
    if (readiness == QStringLiteral("quarantined"))
        r.error = r.error.isEmpty() ? tr("quarantined") : r.error;
    r.state = stateOf(r);
    return r;
}

QString ModelsTable::stateOf(const Row &r)
{
    if (r.downloading)
        return tr("%1 %").arg(qRound(r.progress * 100));
    if (r.warm)
        return tr("warm");
    if (r.ready)
        return tr("idle");
    if (!r.error.isEmpty())
        return tr("failed");
    if (r.bytesDone > 0)
        return tr("partial");
    return tr("not downloaded");
}

void ModelsTable::applyModels(const QString &provider, const QJsonArray &models)
{
    QList<Row> rows;
    for (const Row &r : m_rows) {
        if (r.provider != provider && !(provider == QStringLiteral("speech") && r.provider != QStringLiteral("llm")))
            rows.append(r);
    }
    for (const QJsonValue &v : models) {
        const QJsonObject m = v.toObject();
        if (m.value(QStringLiteral("kind")).toString() == QStringLiteral("vad") || !m.value(QStringLiteral("available")).toBool(true))
            continue;
        rows.append(rowFrom(m));
    }
    // Speech first, the selected row of each provider before the rest.
    std::stable_sort(rows.begin(), rows.end(), [](const Row &a, const Row &b) {
        const int pa = a.provider == QStringLiteral("llm") ? 1 : 0;
        const int pb = b.provider == QStringLiteral("llm") ? 1 : 0;
        return pa != pb ? pa < pb : (a.selected != b.selected && a.selected);
    });
    beginResetModel();
    m_rows = rows;
    applyBackends();
    endResetModel();
    emit countChanged();
    emit summaryChanged();
}

void ModelsTable::applySpeech(const QJsonObject &result)
{
    m_speechDownloads = downloadCount(result.value(QStringLiteral("models")));
    if (m_speechDownloads < 0)
        return;
    const QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    m_modelsDir = format::homePath(result.value(QStringLiteral("models_dir")).toString(), env);
    applyModels(QStringLiteral("speech"), result.value(QStringLiteral("models")).toArray());
}

void ModelsTable::applyLlm(const QJsonObject &result)
{
    m_llmDownloads = downloadCount(result.value(QStringLiteral("models")));
    if (m_llmDownloads < 0)
        return;
    applyModels(QStringLiteral("llm"), result.value(QStringLiteral("models")).toArray());
}

void ModelsTable::applyEngines(const QJsonArray &engines)
{
    for (const QJsonValue &v : engines) {
        const QJsonObject e = v.toObject();
        m_engines.insert(keyOf(e.value(QStringLiteral("binary")).toString()), e);
    }
    applyBackends();
    if (!m_rows.isEmpty())
        emit dataChanged(index(0), index(int(m_rows.size()) - 1));
    emit summaryChanged();
}

void ModelsTable::applyBackends()
{
    for (Row &r : m_rows) {
        const QJsonObject e = m_engines.value(r.provider);
        const QString model = e.value(QStringLiteral("model")).toString();
        const bool running = e.value(QStringLiteral("running")).toBool(false);
        // An engine identifies its loaded file or model directory. Substring
        // matching confuses variants such as tiny and tiny.en.
        const QString loaded = QDir::cleanPath(model);
        const QString expected = QDir::cleanPath(r.path);
        const QFileInfo file(loaded);
        const bool sameModel = !r.path.isEmpty()
                                   ? loaded == expected || loaded.startsWith(expected + QLatin1Char('/'))
                                   : model == r.id || file.dir().dirName() == r.id
                                         || file.fileName() == QStringLiteral("ggml-%1.bin").arg(r.id);
        r.warm = running && r.ready && !model.isEmpty() && sameModel;
        const QString backend = e.value(QStringLiteral("backend")).toString();
        const bool vulkan = backend == QStringLiteral("vulkan")
                            || (backend.isEmpty() && m_gpu == QStringLiteral("vulkan")
                                && r.provider != QStringLiteral("diarize"));
        if (backend.isEmpty() && m_gpu.isEmpty())
            r.backend = tr("not checked");
        else if (backend == QStringLiteral("cuda"))
            r.backend = tr("CUDA");
        else
            r.backend = vulkan ? tr("Vulkan") : tr("CPU");
        r.state = r.ready && !r.downloading && !e.value(QStringLiteral("running")).isBool()
                      ? tr("not checked") : stateOf(r);
    }
}

int ModelsTable::rowOf(const QString &provider, const QString &id) const
{
    for (int i = 0; i < m_rows.size(); ++i) {
        if (m_rows[i].provider == provider && m_rows[i].id == id)
            return i;
    }
    return -1;
}

QString ModelsTable::headline() const
{
    QStringList parts{m_gpu.isEmpty() ? tr("Backend not checked") : (m_gpu == QStringLiteral("vulkan") ? tr("Vulkan") : tr("CPU"))};
    if (!m_modelsDir.isEmpty())
        parts.append(tr("models in %1").arg(m_modelsDir));
    return parts.join(QStringLiteral(" · "));
}

QVariantList ModelsTable::speechChoices() const
{
    QVariantList out;
    for (const Row &r : m_rows) {
        if (r.provider != QStringLiteral("llm"))
            out.append(QVariantMap{{QStringLiteral("key"), r.provider + QLatin1Char('/') + r.id}, {QStringLiteral("label"), r.name}});
    }
    return out;
}

QVariantList ModelsTable::llmChoices() const
{
    QVariantList out;
    for (const Row &r : m_rows) {
        if (r.provider == QStringLiteral("llm") && r.source != QStringLiteral("sideload"))
            out.append(QVariantMap{{QStringLiteral("key"), r.id}, {QStringLiteral("label"), r.name}});
    }
    return out;
}

void ModelsTable::act(const QString &method, const QString &provider, const QString &id, const QJsonObject &extra)
{
    if (m_link == nullptr || !m_link->connected())
        return;
    QJsonObject params = extra;
    if (provider == QStringLiteral("llm"))
        params.insert(QStringLiteral("model"), id);
    else {
        params.insert(QStringLiteral("provider"), provider);
        params.insert(QStringLiteral("model"), id);
    }
    const QString name = (provider == QStringLiteral("llm") ? QStringLiteral("llm.models.") : QStringLiteral("speech.models.")) + method;
    m_link->call(name, params, [this](const QJsonObject &, const QJsonObject &error) {
        answered(error);
        refresh();
    });
}

// The outcome of an action: a refusal stays on the table until the next
// attempt answers, so the person reads why; a success clears it.
void ModelsTable::answered(const QJsonObject &error)
{
    const QString message = error.value(QStringLiteral("message")).toString();
    if (message == m_lastRefusal)
        return;
    m_lastRefusal = message;
    emit refused(m_lastRefusal);
}

void ModelsTable::download(const QString &provider, const QString &id)
{
    act(QStringLiteral("download"), provider, id);
}

void ModelsTable::cancel(const QString &provider, const QString &id)
{
    act(QStringLiteral("cancel"), provider, id);
}

void ModelsTable::remove(const QString &provider, const QString &id)
{
    act(QStringLiteral("delete"), provider, id);
}

void ModelsTable::select(const QString &provider, const QString &id)
{
    if (m_link == nullptr || !m_link->connected())
        return;
    const QJsonObject params{{QStringLiteral("provider"), provider}, {QStringLiteral("model"), id}};
    m_link->call(QStringLiteral("speech.selection.set"), params, [this](const QJsonObject &, const QJsonObject &error) {
        answered(error);
        refresh();
    });
}

void ModelsTable::handleNotification(const QString &topic, const QJsonObject &payload)
{
    if (topic == QStringLiteral("engine.state") || topic == QStringLiteral("config.changed")) {
        if (!m_active && topic == QStringLiteral("config.changed"))
            invalidateDownloads();
        // The daemon's engines row is the truth for `warm` and the backend.
        if (m_active)
            refresh();
        return;
    }
    if (topic != QStringLiteral("model.download"))
        return;
    invalidateDownloads();
    const int row = rowOf(payload.value(QStringLiteral("provider")).toString(), payload.value(QStringLiteral("model")).toString());
    if (row < 0)
        return;
    Row &r = m_rows[row];
    const QString state = payload.value(QStringLiteral("state")).toString();
    r.bytesDone = quint64(payload.value(QStringLiteral("bytes_done")).toDouble());
    r.bytesTotal = quint64(payload.value(QStringLiteral("bytes_total")).toDouble());
    r.progress = r.bytesTotal > 0 ? double(r.bytesDone) / double(r.bytesTotal) : 0.0;
    r.downloading = state == QStringLiteral("running");
    if (!r.downloading) {
        r.error = payload.value(QStringLiteral("error")).toString();
        r.ready = state == QStringLiteral("done");
    }
    r.state = stateOf(r);
    emit dataChanged(index(row), index(row));
    if (!r.downloading && m_active)
        refresh();
}

void ModelsTable::applySample()
{
    m_speechDownloads = m_llmDownloads = 0;
    auto row = [](const char *provider, const char *id, const char *name, const char *detail, const char *size, const char *backend,
                  const char *state, bool ready, bool selected, bool warm, double progress) {
        Row r;
        r.provider = QString::fromUtf8(provider);
        r.id = QString::fromUtf8(id);
        r.name = QString::fromUtf8(name);
        r.detail = QString::fromUtf8(detail);
        r.size = QString::fromUtf8(size);
        r.backend = QString::fromUtf8(backend);
        r.state = QString::fromUtf8(state);
        r.ready = ready;
        r.selected = selected;
        r.warm = warm;
        r.progress = progress;
        r.downloading = progress > 0 && progress < 1;
        return r;
    };
    m_gpu = QStringLiteral("vulkan");
    m_modelsDir = QStringLiteral("~/.local/share/dettivo/models");
    beginResetModel();
    m_rows = {row("parakeet", "parakeet-v3", "Parakeet TDT 0.6B v3 · q8_0", "parakeet.cpp · 25 languages · word timestamps · CC-BY-4.0", "640 MB", "Vulkan", "warm", true, true, true, 0),
              row("parakeet", "parakeet-v2", "Parakeet TDT 0.6B v2 · q8_0", "parakeet.cpp · English · fastest", "650 MB", "Vulkan", "not downloaded", false, false, false, 0),
              row("whisper", "small", "Whisper small", "whisper.cpp · 99 languages · used for meetings", "466 MB", "Vulkan", "idle", true, false, false, 0),
              row("whisper", "large-v3-turbo", "Whisper large-v3-turbo · q8_0", "whisper.cpp · 99 languages · the Mac default", "875 MB", "Vulkan", "38 %", false, false, false, 0.38),
              row("llm", "qwen3-4b-instruct-2507", "Qwen3 4B Instruct 2507 · Q4_K_M", "llama.cpp · Enhanced and meeting analysis · Apache-2.0", "2.5 GB", "Vulkan", "idle", true, true, false, 0),
              row("llm", "qwen3-1.7b", "Qwen3 1.7B · fast", "llama.cpp · Enhanced on a small GPU · optional", "1.1 GB", "Vulkan", "not downloaded", false, false, false, 0)};
    for (const auto &entry : m_rows) {
        if (entry.downloading) {
            if (entry.provider == QStringLiteral("llm"))
                ++m_llmDownloads;
            else
                ++m_speechDownloads;
        }
    }
    endResetModel();
    emit countChanged();
    emit summaryChanged();
}

}  // namespace dettivo
