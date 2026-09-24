// A daemon link for the host tests: canned answers per method, canned
// errors, the calls and params it saw, the notifications a test injects,
// and, with `defer` on, answers held back until `answerPending`, so every
// model is proven without a daemon.
#pragma once

#include "daemon_link.h"

#include <QHash>
#include <QJsonObject>
#include <QList>
#include <QStringList>

#include <utility>

namespace dettivo::test {

class FakeLink : public DaemonLink {
public:
    bool connected() const override { return m_connected; }
    void call(const QString &method, const QJsonObject &params, Reply reply) override
    {
        calls.append(method);
        lastParams.insert(method, params);
        if (defer) {
            pending.append({method, std::move(reply)});
            return;
        }
        answer(method, reply);
    }
    /// Answers every deferred call, in order, with what is canned now.
    void answerPending()
    {
        const auto held = std::move(pending);
        pending.clear();
        for (const auto &[method, reply] : held)
            answer(method, reply);
    }
    /// Answers the newest deferred call alone, with what is canned now,
    /// so a test can hand answers back out of order.
    void answerLastPending()
    {
        if (pending.isEmpty())
            return;
        const auto [method, reply] = pending.takeLast();
        answer(method, reply);
    }
    void subscribe(const QStringList &topics) override { subscriptions.append(topics); }
    void setConnected(bool on)
    {
        m_connected = on;
        emit connectedChanged(on);
    }
    void notify(const QString &topic, const QJsonObject &payload) { emit notification(topic, payload); }

    QStringList calls;
    QHash<QString, QJsonObject> lastParams;
    QList<QStringList> subscriptions;
    QHash<QString, QJsonObject> answers;
    QHash<QString, QJsonObject> errors;
    bool defer = false;
    QList<std::pair<QString, Reply>> pending;

private:
    void answer(const QString &method, const Reply &reply)
    {
        if (reply && errors.contains(method))
            reply({}, errors.value(method));
        else if (reply && answers.contains(method))
            reply(answers.value(method), {});
    }

    bool m_connected = true;
};

}  // namespace dettivo::test
