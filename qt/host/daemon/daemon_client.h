// A Dettivo client's daemon connection, shared by the app and the pill: one
// JSON-RPC line per request over the Unix socket, `events.notify` lines
// dispatched as they arrive, and a reconnect with backoff from half a
// second to eight whenever the daemon is away, so a restart of dettivod
// never needs a restart of the client.
#pragma once

#include "daemon_link.h"

#include <QDeadlineTimer>
#include <QHash>
#include <QLocalSocket>
#include <QTimer>

namespace dettivo {

class DaemonClient : public DaemonLink {
    Q_OBJECT

public:
    explicit DaemonClient(QString socketPath, QString token, QObject *parent = nullptr);
    ~DaemonClient() override;

    bool connected() const override { return m_socket.state() == QLocalSocket::ConnectedState; }
    void call(const QString &method, const QJsonObject &params, Reply reply) override;
    void subscribe(const QStringList &topics) override;
    qint64 requestBytes(const QString &method, const QJsonObject &params) const override
    {
        return encodeRequest(method, params, QStringLiteral("18446744073709551615"), m_token).size();
    }

    /// Connects now and keeps reconnecting until `stop`.
    void start();
    void stop();
    /// Writes what is queued on the socket and waits briefly for it, so a
    /// request sent while the application quits reaches the daemon.
    void finish();

    /// Seconds between attempts, doubling from the first to the cap.
    static constexpr int kFirstBackoffMs = 500;
    static constexpr int kFinishWaitMs = 500;
    static constexpr int kMaxBackoffMs = 8000;
    /// How long a request waits for its answer before its callback is
    /// failed once, naming the wait. The daemon may still have acted on
    /// it: a caller that mutated refreshes the state rather than retrying.
    static constexpr int kRequestTimeoutMs = 60000;
    void setRequestTimeoutMs(int ms) { m_requestTimeoutMs = ms; }

private:
    void onConnected();
    void onDisconnected();
    void onReadyRead();
    void scheduleReconnect();
    void handleLine(const QByteArray &line);
    void failPending(const QString &message);
    void expirePending();

    struct Pending {
        Reply reply;
        QDeadlineTimer due;
    };

    QString m_socketPath;
    QString m_token;
    QLocalSocket m_socket;
    QTimer m_reconnect;
    int m_backoffMs = kFirstBackoffMs;
    bool m_running = false;
    bool m_wasConnected = false;
    quint64 m_nextId = 1;
    QHash<QString, Pending> m_pending;
    QTimer m_expiry;
    int m_requestTimeoutMs = kRequestTimeoutMs;
    QStringList m_topics;
    QByteArray m_buffer;
};

}  // namespace dettivo
