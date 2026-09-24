// What the pill needs from the daemon: a connection that comes and goes,
// one-shot calls, and the notifications of a subscription. DaemonClient
// implements it over the Unix socket; the tests feed a fake so the state
// mapping is proven without a daemon.
#pragma once

#include <QJsonObject>
#include <QJsonDocument>
#include <QObject>
#include <QString>
#include <QStringList>

#include <functional>

namespace dettivo {

class DaemonLink : public QObject {
    Q_OBJECT

public:
    /// The answer to one call: `result` on success, else `error` (the
    /// JSON-RPC error object) or a transport failure in `error["message"]`.
    using Reply = std::function<void(const QJsonObject &result, const QJsonObject &error)>;

    using QObject::QObject;

    virtual bool connected() const = 0;
    /// Sends one request; `reply` runs on the GUI thread when the answer
    /// arrives, or with a transport error when the connection drops.
    virtual void call(const QString &method, const QJsonObject &params, Reply reply) = 0;
    /// Subscribes (again) to `topics` on the current connection.
    virtual void subscribe(const QStringList &topics) = 0;

    static QByteArray encodeRequest(const QString &method, const QJsonObject &params,
                                    const QString &id, const QString &token = {})
    {
        QJsonObject request{{QStringLiteral("jsonrpc"), QStringLiteral("2.0")},
                            {QStringLiteral("id"), id}, {QStringLiteral("method"), method},
                            {QStringLiteral("params"), params}};
        if (!token.isEmpty())
            request.insert(QStringLiteral("auth_token"), token);
        return QJsonDocument(request).toJson(QJsonDocument::Compact) + '\n';
    }
    /// Includes auth and a maximum-width future request id.
    virtual qint64 requestBytes(const QString &method, const QJsonObject &params) const
    {
        return encodeRequest(method, params, QStringLiteral("18446744073709551615")).size();
    }

signals:
    void connectedChanged(bool connected);
    /// The daemon has acknowledged the current event subscription.
    void subscriptionReady();
    /// One `events.notify` line.
    void notification(const QString &topic, const QJsonObject &payload);
    /// `events.overflow`: the subscriber fell behind and events were lost.
    void overflow(qint64 dropped);
};

}  // namespace dettivo
