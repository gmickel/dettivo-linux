// The app's single-instance socket (fn-17 R5): `app.sock` beside the
// daemon socket, one JSON line in (`raise`, `open`, `status`) and one
// line out. A second `dettivo-app` and `dettivo app` forward their request
// here and exit. Ownership is settled before any window exists: the launch
// holding the exclusive lock on `app.lock` is the one instance, the only
// one that listens and the only one that may replace a socket file a
// crash left behind; a launch that finds the lock taken has a sibling
// still starting and forwards to it once it answers.
#pragma once

#include <QJsonObject>
#include <QLocalServer>
#include <QObject>
#include <QString>

#include <functional>

namespace dettivo {

/// The instance lock: `<dir>/app.lock` under an exclusive advisory lock
/// held for the life of this object. One process holds it at a time.
class InstanceLock {
public:
    InstanceLock() = default;
    ~InstanceLock();
    InstanceLock(const InstanceLock &) = delete;
    InstanceLock &operator=(const InstanceLock &) = delete;

    /// Takes the lock; false with `error` when another process holds it
    /// or the directory cannot be used.
    bool acquire(const QString &dir, QString *error);
    bool held() const { return m_fd >= 0; }

    /// `<dir>/app.lock`.
    static QString pathFor(const QString &dir);

private:
    int m_fd = -1;
};

class AppControl : public QObject {
    Q_OBJECT

public:
    using StatusProvider = std::function<QJsonObject()>;

    explicit AppControl(QObject *parent = nullptr);
    ~AppControl() override;

    /// `<dir>/app.sock`.
    static QString socketFor(const QString &dir);

    /// Sends one command to a running app; false when none answers within
    /// `timeoutMs`, with `reply` holding the answer otherwise.
    static bool forward(const QString &dir, const QJsonObject &command, QJsonObject *reply, int timeoutMs = 1000);

    /// Listens on `<dir>/app.sock` for the launch that holds `lock`,
    /// replacing a stale file; a launch without the lock is refused, so it
    /// can never unlink the owner's socket. `error` says why when it cannot.
    bool listen(const QString &dir, const InstanceLock &lock, QString *error);
    QString socketPath() const { return m_path; }

    /// What `status` answers.
    void setStatusProvider(StatusProvider provider) { m_status = std::move(provider); }

    /// One command; the answer is one object with `ok` and, on refusal,
    /// `error`.
    QJsonObject applyCommand(const QJsonObject &command);

signals:
    void raiseRequested();
    void openRequested(const QString &route, const QString &arg);

private:
    void onConnection();

    QLocalServer m_server;
    QString m_path;
    StatusProvider m_status;
};

}  // namespace dettivo
