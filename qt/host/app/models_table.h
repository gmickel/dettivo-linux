// The Models route's table (settings-models.png, ADR 0033): every speech
// model of every provider and every language model of the catalogue in
// one list, with its size, the backend its engine runs on, its state
// (`warm`, `idle`, `not downloaded`, a percentage, `failed`) and the
// actions its state allows; read from `speech.models.status`,
// `llm.models.status` and `speech.engines`, kept current by the
// `model.download` and `engine.state` events. The selections above the
// table are config keys the settings model writes; the choices they
// offer come from here.
#pragma once

#include "daemon_link.h"

#include <QAbstractListModel>
#include <QJsonArray>
#include <QJsonObject>
#include <QString>
#include <QVariantList>

namespace dettivo {

class ModelsTable : public QAbstractListModel {
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
    Q_PROPERTY(QString modelsDir READ modelsDir NOTIFY summaryChanged)
    Q_PROPERTY(QString headline READ headline NOTIFY summaryChanged)
    Q_PROPERTY(QVariantList speechChoices READ speechChoices NOTIFY countChanged)
    Q_PROPERTY(QVariantList llmChoices READ llmChoices NOTIFY countChanged)
    Q_PROPERTY(QString lastRefusal READ lastRefusal NOTIFY refused)

public:
    enum Role {
        KeyRole = Qt::UserRole + 1,
        ProviderRole,
        IdRole,
        NameRole,
        DetailRole,
        SizeRole,
        BackendRole,
        StateRole,
        ProgressRole,
        ReadyRole,
        DownloadingRole,
        SelectedRole,
        WarmRole,
        ErrorRole
    };

    struct Row {
        QString provider, id, name, detail, size, backend, state, error, path, source;
        double progress = 0.0;
        quint64 bytesDone = 0, bytesTotal = 0;
        bool ready = false, downloading = false, selected = false, warm = false;
    };

    explicit ModelsTable(DaemonLink *link, QObject *parent = nullptr);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    /// Re-reads the three answers on every connect and after an action.
    void start();
    /// Refreshes on entry and reconnect only while this settings route is active.
    void setActive(bool active);
    Q_INVOKABLE void refresh();
    /// `speech.models.download`, `speech.models.cancel`, `speech.models.delete`
    /// (`llm.models.*` for the `llm` provider).
    Q_INVOKABLE void download(const QString &provider, const QString &id);
    Q_INVOKABLE void cancel(const QString &provider, const QString &id);
    Q_INVOKABLE void remove(const QString &provider, const QString &id);
    /// `speech.selection.set`: the dictation model.
    Q_INVOKABLE void select(const QString &provider, const QString &id);

    QString modelsDir() const { return m_modelsDir; }
    QString headline() const;
    QVariantList speechChoices() const;
    QVariantList llmChoices() const;
    QList<Row> rows() const { return m_rows; }
    /// -1 until both model catalogues have answered; never infer idle from an empty view.
    int activeDownloads() const;
    void ensureDownloadStatus();
    /// The daemon's last refusal of an action, kept until the next
    /// attempt or a success; a refresh never clears it.
    QString lastRefusal() const { return m_lastRefusal; }

    /// Applies the daemon's answers (tests, the sample).
    void applySpeech(const QJsonObject &result);
    void applyLlm(const QJsonObject &result);
    void applyEngines(const QJsonArray &engines);
    void applySample();

public slots:
    void handleNotification(const QString &topic, const QJsonObject &payload);

signals:
    void countChanged();
    void summaryChanged();
    /// The daemon refused an action; `message` is its reason.
    void refused(const QString &message);

private:
    static Row rowFrom(const QJsonObject &m);
    static QString stateOf(const Row &r);
    void onConnected(bool connected);
    void applyModels(const QString &provider, const QJsonArray &models);
    void applyBackends();
    int rowOf(const QString &provider, const QString &id) const;
    void act(const QString &method, const QString &provider, const QString &id, const QJsonObject &extra = {});
    void answered(const QJsonObject &error);

    void refreshCatalogue();
    void invalidateDownloads();
    static int downloadCount(const QJsonValue &models);
    int m_speechDownloads = -1, m_llmDownloads = -1;
    int m_cataloguePending = 0;
    quint64 m_catalogueGeneration = 0;
    bool m_active = false;
    DaemonLink *m_link;
    QList<Row> m_rows;
    QString m_modelsDir;
    QString m_gpu;
    QString m_lastRefusal;
    QHash<QString, QJsonObject> m_engines;   // provider -> the last speech.engines row
};

}  // namespace dettivo
