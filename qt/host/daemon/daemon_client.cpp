#include "daemon_client.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QLoggingCategory>

namespace dettivo {

Q_LOGGING_CATEGORY(lcDaemonClient, "dettivo.daemon")

DaemonClient::DaemonClient(QString socketPath, QString token, QObject *parent)
    : DaemonLink(parent), m_socketPath(std::move(socketPath)), m_token(std::move(token))
{
    m_reconnect.setSingleShot(true);
    connect(&m_reconnect, &QTimer::timeout, this, [this]() {
        if (m_running && m_socket.state() == QLocalSocket::UnconnectedState)
            m_socket.connectToServer(m_socketPath);
    });
    m_expiry.setInterval(250);
    connect(&m_expiry, &QTimer::timeout, this, &DaemonClient::expirePending);
    connect(&m_socket, &QLocalSocket::connected, this, &DaemonClient::onConnected);
    connect(&m_socket, &QLocalSocket::disconnected, this, &DaemonClient::onDisconnected);
    connect(&m_socket, &QLocalSocket::readyRead, this, &DaemonClient::onReadyRead);
    connect(&m_socket, &QLocalSocket::errorOccurred, this, [this](QLocalSocket::LocalSocketError) {
        if (m_socket.state() != QLocalSocket::ConnectedState)
            onDisconnected();
    });
}

// The socket's own destructor emits disconnected; by then the pending
// table is gone, so the signals are cut first and the socket closed while
// everything it reaches still exists.
DaemonClient::~DaemonClient()
{
    m_running = false;
    m_reconnect.stop();
    m_socket.disconnect(this);
    m_socket.abort();
}

void DaemonClient::start()
{
    m_running = true;
    m_backoffMs = kFirstBackoffMs;
    if (m_socket.state() == QLocalSocket::UnconnectedState)
        m_socket.connectToServer(m_socketPath);
}

void DaemonClient::stop()
{
    m_running = false;
    m_reconnect.stop();
    m_socket.disconnectFromServer();
}

void DaemonClient::finish()
{
    if (!connected())
        return;
    m_socket.flush();
    if (m_socket.bytesToWrite() > 0)
        m_socket.waitForBytesWritten(kFinishWaitMs);
}

void DaemonClient::onConnected()
{
    qCInfo(lcDaemonClient) << "connected to" << m_socketPath;
    m_backoffMs = kFirstBackoffMs;
    m_buffer.clear();
    m_wasConnected = true;
    emit connectedChanged(true);
    if (!m_topics.isEmpty())
        subscribe(m_topics);
}

// Reached on a real disconnect and on every failed reconnect attempt;
// only the former is a change the model needs to hear about.
void DaemonClient::onDisconnected()
{
    failPending(QStringLiteral("the daemon connection closed"));
    m_buffer.clear();
    if (m_wasConnected) {
        m_wasConnected = false;
        qCInfo(lcDaemonClient) << "daemon away; reconnecting";
        emit connectedChanged(false);
    }
    scheduleReconnect();
}

void DaemonClient::scheduleReconnect()
{
    if (!m_running || m_reconnect.isActive())
        return;
    m_reconnect.start(m_backoffMs);
    m_backoffMs = qMin(m_backoffMs * 2, kMaxBackoffMs);
}

void DaemonClient::call(const QString &method, const QJsonObject &params, Reply reply)
{
    if (!connected()) {
        if (reply)
            reply({}, QJsonObject{{QStringLiteral("message"), QStringLiteral("not connected")}});
        return;
    }
    const QString id = QString::number(m_nextId++);
    const QByteArray line = encodeRequest(method, params, id, m_token);
    if (m_socket.write(line) != line.size()) {
        // A write the socket refuses completes the callback here, through
        // the same path a reply or an expiry would.
        if (reply)
            reply({}, QJsonObject{{QStringLiteral("message"), QStringLiteral("the request could not be written to the daemon")}});
        return;
    }
    if (reply) {
        m_pending.insert(id, Pending{std::move(reply), QDeadlineTimer(m_requestTimeoutMs)});
        if (!m_expiry.isActive())
            m_expiry.start();
    }
}

// Every request past its deadline is failed exactly once; the entry is
// gone before the callback runs, so a late reply finds nothing to answer.
void DaemonClient::expirePending()
{
    QStringList expired;
    for (auto it = m_pending.cbegin(); it != m_pending.cend(); ++it) {
        if (it.value().due.hasExpired())
            expired.append(it.key());
    }
    for (const QString &key : expired) {
        const Pending pending = m_pending.take(key);
        qCWarning(lcDaemonClient) << "request" << key << "unanswered after" << m_requestTimeoutMs << "ms";
        pending.reply({}, QJsonObject{{QStringLiteral("message"),
                                       QStringLiteral("no answer from the daemon within %1 ms; the request may still have taken effect")
                                           .arg(m_requestTimeoutMs)}});
    }
    if (m_pending.isEmpty())
        m_expiry.stop();
}

void DaemonClient::subscribe(const QStringList &topics)
{
    m_topics = topics;
    if (!connected())
        return;
    QJsonArray list;
    for (const QString &t : topics)
        list.append(t);
    call(QStringLiteral("events.subscribe"),
         QJsonObject{{QStringLiteral("topics"), list}, {QStringLiteral("buffer"), 256}},
         [this](const QJsonObject &, const QJsonObject &error) {
             if (!error.isEmpty())
                 qCWarning(lcDaemonClient) << "events.subscribe refused:" << error;
             else
                 emit subscriptionReady();
         });
}

void DaemonClient::onReadyRead()
{
    m_buffer.append(m_socket.readAll());
    int newline = -1;
    while ((newline = int(m_buffer.indexOf('\n'))) >= 0) {
        const QByteArray line = m_buffer.left(newline);
        m_buffer.remove(0, newline + 1);
        if (!line.trimmed().isEmpty())
            handleLine(line);
    }
}

void DaemonClient::handleLine(const QByteArray &line)
{
    const QJsonDocument doc = QJsonDocument::fromJson(line);
    if (!doc.isObject())
        return;
    const QJsonObject message = doc.object();
    if (message.value(QStringLiteral("method")).toString() == QStringLiteral("events.notify")) {
        const QJsonObject params = message.value(QStringLiteral("params")).toObject();
        const QString topic = params.value(QStringLiteral("topic")).toString();
        const QJsonObject payload = params.value(QStringLiteral("payload")).toObject();
        if (topic == QStringLiteral("events.overflow"))
            emit overflow(payload.value(QStringLiteral("dropped")).toInteger());
        else
            emit notification(topic, payload);
        return;
    }
    const QJsonValue id = message.value(QStringLiteral("id"));
    const QString key = id.isString() ? id.toString() : QString::number(id.toInteger());
    const Pending pending = m_pending.take(key);
    if (m_pending.isEmpty())
        m_expiry.stop();
    if (!pending.reply)
        return;
    if (message.contains(QStringLiteral("error")))
        pending.reply({}, message.value(QStringLiteral("error")).toObject());
    else
        pending.reply(message.value(QStringLiteral("result")).toObject(), {});
}

void DaemonClient::failPending(const QString &message)
{
    const auto pending = std::move(m_pending);
    m_pending.clear();
    m_expiry.stop();
    for (const Pending &entry : pending)
        entry.reply({}, QJsonObject{{QStringLiteral("message"), message}});
}

}  // namespace dettivo
