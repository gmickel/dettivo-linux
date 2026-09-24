#include "app_control.h"
#include "router.h"

#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QLocalSocket>
#include <QLoggingCategory>

#include <cerrno>
#include <cstring>
#include <fcntl.h>
#include <sys/file.h>
#include <unistd.h>

namespace dettivo {

Q_LOGGING_CATEGORY(lcAppControl, "dettivo.app.control")

InstanceLock::~InstanceLock()
{
    if (m_fd >= 0)
        ::close(m_fd);
}

QString InstanceLock::pathFor(const QString &dir)
{
    return dir + QStringLiteral("/app.lock");
}

bool InstanceLock::acquire(const QString &dir, QString *error)
{
    if (held())
        return true;
    if (!QDir().mkpath(dir)) {
        if (error != nullptr)
            *error = QStringLiteral("cannot create %1").arg(dir);
        return false;
    }
    const QByteArray path = QFile::encodeName(pathFor(dir));
    const int fd = ::open(path.constData(), O_RDWR | O_CREAT | O_CLOEXEC, 0600);
    if (fd < 0) {
        if (error != nullptr)
            *error = QStringLiteral("%1: %2").arg(QString::fromUtf8(path), QString::fromLocal8Bit(std::strerror(errno)));
        return false;
    }
    if (::flock(fd, LOCK_EX | LOCK_NB) != 0) {
        const int why = errno;
        ::close(fd);
        if (error != nullptr)
            *error = why == EWOULDBLOCK ? QStringLiteral("another dettivo-app holds %1").arg(QString::fromUtf8(path))
                                        : QStringLiteral("%1: %2").arg(QString::fromUtf8(path), QString::fromLocal8Bit(std::strerror(why)));
        return false;
    }
    m_fd = fd;
    return true;
}

AppControl::AppControl(QObject *parent) : QObject(parent)
{
    m_server.setSocketOptions(QLocalServer::UserAccessOption);
    connect(&m_server, &QLocalServer::newConnection, this, &AppControl::onConnection);
}

AppControl::~AppControl()
{
    m_server.close();
    if (!m_path.isEmpty())
        QLocalServer::removeServer(m_path);
}

QString AppControl::socketFor(const QString &dir)
{
    return dir + QStringLiteral("/app.sock");
}

bool AppControl::forward(const QString &dir, const QJsonObject &command, QJsonObject *reply, int timeoutMs)
{
    const QString path = socketFor(dir);
    if (!QFile::exists(path))
        return false;
    QLocalSocket socket;
    socket.connectToServer(path);
    if (!socket.waitForConnected(timeoutMs))
        return false;
    socket.write(QJsonDocument(command).toJson(QJsonDocument::Compact) + '\n');
    if (!socket.waitForBytesWritten(timeoutMs))
        return false;
    QByteArray line;
    while (!line.contains('\n')) {
        if (!socket.waitForReadyRead(timeoutMs))
            return false;
        line.append(socket.readAll());
    }
    const QJsonDocument doc = QJsonDocument::fromJson(line.left(line.indexOf('\n')));
    if (!doc.isObject())
        return false;
    if (reply != nullptr)
        *reply = doc.object();
    return true;
}

bool AppControl::listen(const QString &dir, const InstanceLock &lock, QString *error)
{
    if (!lock.held()) {
        if (error != nullptr)
            *error = QStringLiteral("this launch does not hold the instance lock; another dettivo-app is starting");
        return false;
    }
    if (!QDir().mkpath(dir)) {
        if (error != nullptr)
            *error = QStringLiteral("cannot create %1").arg(dir);
        return false;
    }
    QFile::setPermissions(dir, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
    m_path = socketFor(dir);
    // Only the lock holder gets here, and it has already tried to reach a
    // live app on this path: a file nobody answers on is a crash's
    // leftover, never a sibling's socket.
    QLocalServer::removeServer(m_path);
    if (!m_server.listen(m_path)) {
        if (error != nullptr)
            *error = QStringLiteral("%1: %2").arg(m_path, m_server.errorString());
        m_path.clear();
        return false;
    }
    qCInfo(lcAppControl) << "instance socket at" << m_path;
    return true;
}

QJsonObject AppControl::applyCommand(const QJsonObject &command)
{
    const QString cmd = command.value(QStringLiteral("cmd")).toString();
    if (cmd == QStringLiteral("raise")) {
        emit raiseRequested();
        return {{QStringLiteral("ok"), true}};
    }
    if (cmd == QStringLiteral("open")) {
        const QString name = command.value(QStringLiteral("route")).toString();
        QString route, sub, error;
        if (!Router::parse(name, &route, &sub, &error))
            return {{QStringLiteral("ok"), false}, {QStringLiteral("error"), error}};
        emit openRequested(Router::pageFor(route, sub), command.value(QStringLiteral("arg")).toString());
        emit raiseRequested();
        return {{QStringLiteral("ok"), true}, {QStringLiteral("route"), Router::pageFor(route, sub)}};
    }
    if (cmd == QStringLiteral("status")) {
        QJsonObject status = m_status ? m_status() : QJsonObject();
        status.insert(QStringLiteral("ok"), true);
        return status;
    }
    return {{QStringLiteral("ok"), false},
            {QStringLiteral("error"), QStringLiteral("unknown command; the commands are raise, open, status")}};
}

void AppControl::onConnection()
{
    while (QLocalSocket *socket = m_server.nextPendingConnection()) {
        auto *buffer = new QByteArray();
        connect(socket, &QLocalSocket::disconnected, socket, &QObject::deleteLater);
        connect(socket, &QObject::destroyed, socket, [buffer]() { delete buffer; });
        connect(socket, &QLocalSocket::readyRead, this, [this, socket, buffer]() {
            buffer->append(socket->readAll());
            const auto newline = buffer->indexOf('\n');
            if (newline < 0)
                return;
            const QJsonDocument doc = QJsonDocument::fromJson(buffer->left(newline));
            QJsonObject reply;
            if (doc.isObject()) {
                reply = applyCommand(doc.object());
            } else {
                reply.insert(QStringLiteral("ok"), false);
                reply.insert(QStringLiteral("error"), QStringLiteral("expected one JSON object per line"));
            }
            socket->write(QJsonDocument(reply).toJson(QJsonDocument::Compact) + '\n');
            socket->flush();
            socket->disconnectFromServer();
        });
    }
}

}  // namespace dettivo
