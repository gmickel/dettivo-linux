// What the history detail does to an item (fn-22 R2, R3): a re-run
// through `transcripts.rerun` with a chosen engine and model, an export
// through a download transfer written to a file the portal file chooser
// names (or the QA export directory), and a delete through
// `transcripts.delete`. Every outcome is a signal the screens and the
// list follow; every refusal carries the daemon's reason.
#pragma once

#include "daemon_link.h"

#include <QJsonObject>
#include <QObject>
#include <QSaveFile>

#include <memory>
#include <QString>
#include <QVariantList>
#include <QVariantMap>

namespace dettivo {

class HistoryActions : public QObject {
    Q_OBJECT
    Q_PROPERTY(QVariantList providers READ providers NOTIFY providersChanged)
    Q_PROPERTY(QString selectedProvider READ selectedProvider NOTIFY providersChanged)
    Q_PROPERTY(QString selectedModel READ selectedModel NOTIFY providersChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(QString exportDir READ exportDir CONSTANT)

public:
    explicit HistoryActions(DaemonLink *link, QString exportDir, QObject *parent = nullptr);

    /// Reads `speech.providers.list` and `speech.selection.get` for the
    /// re-run dialog's pickers.
    Q_INVOKABLE void loadProviders();
    /// `transcripts.rerun` with the engine and model chosen (empty keeps
    /// the item's engine and the selection in force).
    Q_INVOKABLE void rerun(const QString &id, const QString &provider, const QString &model);
    /// Exports one item (`scope = item`) or everything (`scope = all`) in
    /// `format` (`json`, `md`, `txt`, `zip`) to a file.
    Q_INVOKABLE void exportItems(const QString &id, const QString &format, const QString &scope);
    /// Exports one meeting in `format` (`txt`, `md`, `srt`, `vtt`, `json`)
    /// through the same transfer path; `raw` renders the engine's words
    /// and a present `notesOverride` (including empty) replaces saved notes.
    Q_INVOKABLE void exportMeeting(const QString &id, const QString &format, bool raw, const QVariant &notesOverride = {});
    /// `transcripts.delete`.
    Q_INVOKABLE void remove(const QString &id);
    /// Copies the displayed transcript without targeting a window.
    Q_INVOKABLE bool copyText(const QString &text) const;

    /// `{id, label, models: [{id, label, downloaded}]}` per provider.
    QVariantList providers() const { return m_providers; }
    QString selectedProvider() const { return m_selectedProvider; }
    QString selectedModel() const { return m_selectedModel; }
    bool busy() const { return m_busy; }
    /// `DETTIVO_E2E_EXPORT_DIR`; empty means the portal file chooser.
    QString exportDir() const { return m_exportDir; }
    /// Fills the pickers from the two results (tests, the sample).
    void applyProviders(const QJsonObject &providers, const QJsonObject &selection);
    /// The content type of an export format.
    static QString contentType(const QString &format);

signals:
    /// A re-run started: the new item and the job to follow.
    void rerunStarted(const QString &itemId, const QString &newItemId, const QString &jobId);
    /// The export landed in `path` (`bytes` long).
    void exported(const QString &path, qint64 bytes);
    /// The item is gone from the store.
    void removed(const QString &id);
    /// The daemon (or the file system) refused `action` (`rerun`,
    /// `export`, `delete`) with `reason`.
    void failed(const QString &action, const QString &reason);
    void providersChanged();
    void busyChanged();

private slots:
    /// The portal request's `Response`: 0 with the chosen uri, else cancelled.
    void portalResponse(uint code, const QVariantMap &results);

private:
    struct Transfer {
        QString id, filename, path;
        qint64 bytes = 0;
        quint64 seq = 1;
        // The destination, written beside the previous file and committed
        // only once the whole transfer landed; dropped on failure.
        std::shared_ptr<QSaveFile> file;
    };
    void setBusy(bool busy);
    void pullChunks(Transfer transfer);
    void chooseTarget(Transfer transfer);
    void beginExport(QJsonObject params);
    void writeChunk(Transfer transfer, const QByteArray &data, bool eof);
    bool ready(const QString &action);

    DaemonLink *m_link;
    QString m_exportDir;
    QVariantList m_providers;
    QString m_selectedProvider, m_selectedModel;
    Transfer m_pending;
    bool m_busy = false;
};

}  // namespace dettivo
