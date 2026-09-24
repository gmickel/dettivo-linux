#include "engines_model.h"
#include "status_format.h"

#include <QJsonValue>

namespace dettivo {

namespace {

const QStringList kKeys = {QStringLiteral("whisper"), QStringLiteral("parakeet"), QStringLiteral("llm"),
                           QStringLiteral("diarize")};

}  // namespace

EnginesModel::EnginesModel(DaemonLink *link, QObject *parent) : QAbstractListModel(parent), m_link(link)
{
    if (m_link != nullptr) {
        connect(m_link, &DaemonLink::notification, this, &EnginesModel::handleNotification);
        connect(m_link, &DaemonLink::connectedChanged, this, [this](bool connected) {
            if (connected)
                refresh();
        });
    }
    rebuild();
}

int EnginesModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : int(m_engines.size());
}

QVariant EnginesModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_engines.size())
        return {};
    const Engine &e = m_engines.at(index.row());
    switch (role) {
    case KeyRole:
        return e.key;
    case NameRole:
        return e.name;
    case DetailRole:
        return e.detail;
    case StateRole:
        return e.state;
    case ProgressRole:
        return e.progress;
    case BusyRole:
        return e.busy;
    default:
        return {};
    }
}

QHash<int, QByteArray> EnginesModel::roleNames() const
{
    return {{KeyRole, "key"},     {NameRole, "name"},         {DetailRole, "detail"},
            {StateRole, "state"}, {ProgressRole, "progress"}, {BusyRole, "busy"}};
}

QString EnginesModel::keyOf(const QString &binary)
{
    for (const QString &key : kKeys) {
        if (binary.endsWith(QStringLiteral("-") + key))
            return key;
    }
    return QString();
}

// `whisper large-v3-turbo`; a model id that already starts with the
// engine's name (`parakeet-v3-int8`) reads as `parakeet v3-int8`.
QString EnginesModel::speechName(const QString &key, const QString &model)
{
    if (model.isEmpty())
        return key;
    const QString prefix = key + QLatin1Char('-');
    const QString rest = model.startsWith(prefix, Qt::CaseInsensitive) ? model.mid(prefix.size()) : model;
    return key + QLatin1Char(' ') + rest;
}

int EnginesModel::rowOf(const QString &key) const
{
    for (int i = 0; i < m_engines.size(); ++i) {
        if (m_engines[i].key == key)
            return i;
    }
    return -1;
}

void EnginesModel::refresh()
{
    if (m_link == nullptr || !m_link->connected())
        return;
    m_link->call(QStringLiteral("speech.selection.get"), {}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty())
            applySelection(result);
    });
    m_link->call(QStringLiteral("speech.engines"), {}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty())
            applyEngines(result.value(QStringLiteral("engines")).toArray());
    });
}

void EnginesModel::applySelection(const QJsonObject &selection)
{
    const QJsonObject dictation = selection.value(QStringLiteral("dictation")).toObject();
    m_speechProvider = dictation.value(QStringLiteral("is_parakeet")).toBool(false) ? QStringLiteral("parakeet")
                                                                                     : QStringLiteral("whisper");
    m_speechModel = format::modelName(dictation.value(QStringLiteral("model_id")).toString());
    rebuild();
}

void EnginesModel::applyEngines(const QJsonArray &engines)
{
    for (const QJsonValue &value : engines) {
        const QJsonObject engine = value.toObject();
        const QString key = keyOf(engine.value(QStringLiteral("binary")).toString());
        if (key.isEmpty())
            continue;
        QJsonObject facts = m_reported.value(key);
        const bool running = engine.value(QStringLiteral("running")).toBool(false);
        const bool installed = !engine.value(QStringLiteral("path")).isNull();
        QString state = running ? QStringLiteral("warm") : (installed ? QStringLiteral("idle") : QStringLiteral("not installed"));
        if (engine.value(QStringLiteral("degraded")).toBool(false))
            state = QStringLiteral("cpu");
        if (engine.value(QStringLiteral("crashes")).toInt(0) > 0 && !running)
            state = QStringLiteral("crashed");
        facts.insert(QStringLiteral("state"), state);
        facts.insert(QStringLiteral("model"), format::modelName(engine.value(QStringLiteral("model")).toString()));
        facts.insert(QStringLiteral("backend"), engine.value(QStringLiteral("backend")).toString());
        m_reported.insert(key, facts);
    }
    rebuild();
}

void EnginesModel::rebuild()
{
    const QString other = m_speechProvider == QStringLiteral("whisper") ? QStringLiteral("parakeet") : QStringLiteral("whisper");
    const QStringList order = {m_speechProvider, other, QStringLiteral("llm"), QStringLiteral("diarize")};
    QList<Engine> rows;
    for (const QString &key : order) {
        const QJsonObject facts = m_reported.value(key);
        Engine e;
        e.key = key;
        const QString model = facts.value(QStringLiteral("model")).toString();
        if (key == QStringLiteral("llm"))
            e.name = model.isEmpty() ? tr("language model") : model;
        else if (key == QStringLiteral("diarize"))
            e.name = tr("diarization");
        else {
            const QString shown = !model.isEmpty() ? model : (key == m_speechProvider ? m_speechModel : QString());
            e.name = speechName(key, shown);
        }
        e.state = facts.value(QStringLiteral("state")).toString(QStringLiteral("idle"));
        e.detail = facts.value(QStringLiteral("backend")).toString();
        e.progress = facts.value(QStringLiteral("progress")).toDouble(0.0);
        e.busy = facts.value(QStringLiteral("busy")).toBool(false);
        rows.append(e);
    }
    beginResetModel();
    m_engines = rows;
    endResetModel();
    emit countChanged();
    emit summaryChanged();
}

void EnginesModel::updateRow(int row)
{
    if (row < 0 || row >= m_engines.size())
        return;
    const QJsonObject facts = m_reported.value(m_engines[row].key);
    Engine &e = m_engines[row];
    e.state = facts.value(QStringLiteral("state")).toString(e.state);
    e.detail = facts.value(QStringLiteral("backend")).toString();
    e.progress = facts.value(QStringLiteral("progress")).toDouble(0.0);
    e.busy = facts.value(QStringLiteral("busy")).toBool(false);
    const QString model = facts.value(QStringLiteral("model")).toString();
    if (!model.isEmpty() && e.key != QStringLiteral("diarize"))
        e.name = e.key == QStringLiteral("llm") ? model : speechName(e.key, model);
    emit dataChanged(index(row), index(row));
    emit summaryChanged();
}

void EnginesModel::handleNotification(const QString &topic, const QJsonObject &payload)
{
    if (topic == QStringLiteral("config.changed")) {
        refresh();
        return;
    }
    if (topic == QStringLiteral("engine.state")) {
        const QString key = keyOf(payload.value(QStringLiteral("binary")).toString());
        if (key.isEmpty())
            return;
        QJsonObject facts = m_reported.value(key);
        const QString state = payload.value(QStringLiteral("state")).toString();
        if (state == QStringLiteral("spawned")) {
            facts.insert(QStringLiteral("state"), QStringLiteral("loading"));
            facts.insert(QStringLiteral("busy"), true);
        } else if (state == QStringLiteral("loaded")) {
            facts.insert(QStringLiteral("state"), QStringLiteral("warm"));
            facts.insert(QStringLiteral("busy"), false);
            facts.insert(QStringLiteral("progress"), 1.0);
        } else if (state == QStringLiteral("unloaded")) {
            facts.insert(QStringLiteral("state"), QStringLiteral("idle"));
            facts.insert(QStringLiteral("busy"), false);
            facts.insert(QStringLiteral("progress"), 0.0);
        } else if (state == QStringLiteral("crashed")) {
            facts.insert(QStringLiteral("state"), QStringLiteral("crashed"));
            facts.insert(QStringLiteral("busy"), false);
        } else if (state == QStringLiteral("degraded")) {
            facts.insert(QStringLiteral("state"), QStringLiteral("cpu"));
        }
        const QString model = format::modelName(payload.value(QStringLiteral("model")).toString());
        if (!model.isEmpty())
            facts.insert(QStringLiteral("model"), model);
        if (!payload.value(QStringLiteral("backend")).isNull())
            facts.insert(QStringLiteral("backend"), payload.value(QStringLiteral("backend")).toString());
        m_reported.insert(key, facts);
        updateRow(rowOf(key));
        return;
    }
    if (topic == QStringLiteral("model.download")) {
        const QString provider = payload.value(QStringLiteral("provider")).toString();
        const QString key = provider == QStringLiteral("parakeet") ? provider : QStringLiteral("whisper");
        QJsonObject facts = m_reported.value(key);
        const double total = payload.value(QStringLiteral("bytes_total")).toDouble();
        const double done = payload.value(QStringLiteral("bytes_done")).toDouble();
        const bool running = payload.value(QStringLiteral("state")).toString() == QStringLiteral("running");
        facts.insert(QStringLiteral("busy"), running);
        facts.insert(QStringLiteral("progress"), running && total > 0 ? qBound(0.0, done / total, 1.0) : 0.0);
        if (running)
            facts.insert(QStringLiteral("state"), QStringLiteral("downloading"));
        m_reported.insert(key, facts);
        updateRow(rowOf(key));
        return;
    }
    if (topic == QStringLiteral("job.progress")) {
        const int row = rowOf(m_speechProvider);
        if (row < 0)
            return;
        QJsonObject facts = m_reported.value(m_speechProvider);
        const double progress = payload.value(QStringLiteral("progress")).toDouble();
        facts.insert(QStringLiteral("progress"), qBound(0.0, progress, 1.0));
        facts.insert(QStringLiteral("busy"), progress < 1.0);
        m_reported.insert(m_speechProvider, facts);
        updateRow(row);
    }
}

QString EnginesModel::speechName() const
{
    return m_engines.isEmpty() ? QString() : m_engines.first().name;
}

QString EnginesModel::speechState() const
{
    return m_engines.isEmpty() ? QString() : m_engines.first().state;
}

QString EnginesModel::languageModelName() const
{
    const int row = rowOf(QStringLiteral("llm"));
    return row < 0 ? QString() : m_engines[row].name;
}

QString EnginesModel::languageModelState() const
{
    const int row = rowOf(QStringLiteral("llm"));
    return row < 0 ? QString() : m_engines[row].state;
}

}  // namespace dettivo
