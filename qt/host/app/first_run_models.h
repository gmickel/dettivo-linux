// The Models step's rows (FR-U3, ADR 0024): the speech models first run
// offers, from `speech.models.status` (the rows with a `recommended_for`
// line, plus any model already ready or selected), with size, languages,
// the recommendation, readiness and download progress from the
// `model.download` events; the tier picks the default; a pick writes
// `[speech]` through `speech.selection.set` and starts the verified
// download; the Enhanced tick writes `[llm] provider` and offers the
// local engine's model through `llm.models.download` once that method
// is declared, Ollama detection through `llm.providers.list` until then.
#pragma once

#include "daemon_link.h"

#include <QAbstractListModel>
#include <QElapsedTimer>
#include <QJsonArray>
#include <QJsonObject>
#include <QString>

namespace dettivo {

class FirstRunModels : public QAbstractListModel {
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
    Q_PROPERTY(bool ready READ ready NOTIFY summaryChanged)
    Q_PROPERTY(QString selectedKey READ selectedKey NOTIFY summaryChanged)
    Q_PROPERTY(QString modelsDir READ modelsDir NOTIFY summaryChanged)
    Q_PROPERTY(QString tierLine READ tierLine NOTIFY summaryChanged)
    Q_PROPERTY(bool downloading READ downloading NOTIFY summaryChanged)
    Q_PROPERTY(double downloadFraction READ downloadFraction NOTIFY summaryChanged)
    Q_PROPERTY(QString downloadFile READ downloadFile NOTIFY summaryChanged)
    Q_PROPERTY(QString downloadLine READ downloadLine NOTIFY summaryChanged)
    Q_PROPERTY(QString enhanced READ enhanced NOTIFY enhancedChanged)
    Q_PROPERTY(QString localTitle READ localTitle NOTIFY enhancedChanged)
    Q_PROPERTY(QString localLine READ localLine NOTIFY enhancedChanged)
    Q_PROPERTY(QString localSize READ localSize NOTIFY enhancedChanged)
    Q_PROPERTY(QString localState READ localState NOTIFY enhancedChanged)
    Q_PROPERTY(QString ollamaLine READ ollamaLine NOTIFY enhancedChanged)
    Q_PROPERTY(bool ollamaAvailable READ ollamaAvailable NOTIFY enhancedChanged)

public:
    enum Role {
        KeyRole = Qt::UserRole + 1,
        ProviderRole,
        IdRole,
        NameRole,
        DetailRole,
        SizeRole,
        StateRole,
        ProgressRole,
        RecommendedRole,
        SelectedRole,
        ReadyRole,
        ErrorRole
    };

    struct Row {
        QString provider, id, name, detail, size, state, error;
        double progress = 0.0;
        bool recommended = false;
        bool selected = false;
        bool ready = false;
        bool downloading = false;
        quint64 bytesDone = 0, bytesTotal = 0;
        QString quantization;
    };

    explicit FirstRunModels(DaemonLink *link, QObject *parent = nullptr);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    /// Re-reads the rows, the tier and the providers.
    void refresh(bool includeSpeech = true);
    /// The rows first run offers, in catalogue order.
    QList<Row> rows() const { return m_rows; }

    bool ready() const;
    QString selectedKey() const;
    QString modelsDir() const { return m_modelsDir; }
    QString tierLine() const;
    bool downloading() const;
    double downloadFraction() const;
    QString downloadFile() const;
    QString downloadLine() const;
    QString enhanced() const { return m_enhanced; }
    QString localTitle() const { return m_localTitle; }
    QString localLine() const { return m_localLine; }
    QString localSize() const { return m_localSize; }
    QString localState() const { return m_localState; }
    QString ollamaLine() const { return m_ollamaLine; }
    bool ollamaAvailable() const { return m_ollamaAvailable; }

    /// Picks a row: writes `[speech]`, then downloads it unless it is on disk.
    Q_INVOKABLE void select(const QString &provider, const QString &id);
    /// Cancels a running download; the partial file stays for a resume.
    Q_INVOKABLE void cancel(const QString &provider, const QString &id);
    /// Starts (again) a download that failed or was cancelled.
    Q_INVOKABLE void download(const QString &provider, const QString &id);
    /// `local`, `ollama` or `none`: writes `[llm] provider`; `local` also
    /// starts the catalogue download of `[llm] model` when the daemon
    /// offers it and the model is not on disk.
    Q_INVOKABLE void setEnhanced(const QString &choice);

    /// The GPU tier from `system.capabilities.platform.gpu`.
    void setTier(const QString &gpu);
    /// The `llm.methods` the daemon declares; `llm.models.download` among
    /// them offers the local model's download.
    void setLlmMethods(const QStringList &methods);
    /// Applies a `speech.models.status` answer (tests, the sample).
    void applyStatus(const QJsonObject &result);
    /// Applies `llm.providers.list` and the `llm` capability block.
    void applyProviders(const QJsonArray &providers, const QStringList &llmMethods);
    /// Applies the `[llm] provider` value in force.
    void applyLlmProvider(const QString &provider);
    /// The default pick for the tier (`parakeet-v3` on Vulkan, `small.en`
    /// on the CPU) when nothing is selected or ready.
    static QString defaultFor(const QString &gpu);

public slots:
    void handleNotification(const QString &topic, const QJsonObject &payload);

signals:
    void countChanged();
    void summaryChanged();
    void enhancedChanged();

private:
    int rowOf(const QString &provider, const QString &id) const;
    /// `llm.models.download` for `[llm] model` after `llm.models.status`
    /// named it; a refusal stays on the local row.
    void downloadLocalModel();
    /// A `model.download` event for the local language model.
    void applyLocalDownload(const QJsonObject &payload);
    void updateRow(int row);
    void writeSelection(const QString &provider, const QString &id);

    DaemonLink *m_link;
    QList<Row> m_rows;
    QString m_modelsDir;
    QString m_gpu;
    QString m_enhanced = QStringLiteral("none");
    QString m_localTitle, m_localLine, m_localSize, m_localState;
    QString m_ollamaLine;
    bool m_ollamaAvailable = false;
    bool m_localDownloadOffered = false;
    QStringList m_llmMethods;
    /// The last `llm.providers.list` answer, re-read when the capabilities
    /// arrive after it.
    QJsonArray m_providers;
    /// The catalogue model `[llm] model` names, once `llm.models.status`
    /// answered; what `setEnhanced("local")` downloads.
    QString m_localModel;
    QElapsedTimer m_rate;
    quint64 m_rateBytes = 0;
    double m_bytesPerSecond = 0.0;
};

}  // namespace dettivo
