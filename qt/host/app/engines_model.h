// The four engines of the right rail and the sidebar footer (fn-17 R2):
// the selected speech engine first, the other one, the language model and
// diarization, each with its model, its state (`warm`, `idle`, `cpu`,
// `crashed`, `not installed`) and a progress fraction for the 2 px
// hairline. Read from `speech.engines` and `speech.selection.get`, kept
// current by `engine.state`, `job.progress` and `model.download`.
#pragma once

#include "daemon_link.h"

#include <QAbstractListModel>
#include <QJsonArray>
#include <QJsonObject>
#include <QString>

namespace dettivo {

class EnginesModel : public QAbstractListModel {
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
    Q_PROPERTY(QString speechName READ speechName NOTIFY summaryChanged)
    Q_PROPERTY(QString speechState READ speechState NOTIFY summaryChanged)
    Q_PROPERTY(QString languageModelName READ languageModelName NOTIFY summaryChanged)
    Q_PROPERTY(QString languageModelState READ languageModelState NOTIFY summaryChanged)

public:
    enum Role { KeyRole = Qt::UserRole + 1, NameRole, DetailRole, StateRole, ProgressRole, BusyRole };

    struct Engine {
        QString key;      // whisper, parakeet, llm, diarize
        QString name;     // `whisper large-v3-turbo`
        QString detail;   // the backend, when loaded
        QString state;    // warm, idle, loading, cpu, crashed, not installed
        double progress = 0.0;
        bool busy = false;
    };

    explicit EnginesModel(DaemonLink *link, QObject *parent = nullptr);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    /// Re-reads the engines and the selection.
    void refresh();
    /// The rows in rail order.
    QList<Engine> engines() const { return m_engines; }

    QString speechName() const;
    QString speechState() const;
    QString languageModelName() const;
    QString languageModelState() const;

    /// Applies `speech.engines` and `speech.selection.get` answers (tests,
    /// the sample data).
    void applyEngines(const QJsonArray &engines);
    void applySelection(const QJsonObject &selection);

public slots:
    void handleNotification(const QString &topic, const QJsonObject &payload);

signals:
    void countChanged();
    void summaryChanged();

private:
    static QString keyOf(const QString &binary);
    static QString speechName(const QString &key, const QString &model);
    int rowOf(const QString &key) const;
    void rebuild();
    void updateRow(int row);

    DaemonLink *m_link;
    QList<Engine> m_engines;
    QString m_speechProvider = QStringLiteral("whisper");
    QString m_speechModel;
    QHash<QString, QJsonObject> m_reported;   // key -> the last speech.engines or engine.state facts
};

}  // namespace dettivo
