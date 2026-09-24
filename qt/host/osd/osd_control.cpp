#include "osd_control.h"
#include "osd_model.h"

#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QLocalSocket>
#include <QLoggingCategory>
#include <QSaveFile>

namespace dettivo {

Q_LOGGING_CATEGORY(lcOsdControl, "dettivo.osd.control")

OsdControl::OsdControl(OsdModel *model, QObject *parent) : QObject(parent), m_model(model)
{
    m_server.setSocketOptions(QLocalServer::UserAccessOption);
    connect(&m_server, &QLocalServer::newConnection, this, &OsdControl::onConnection);
}

OsdControl::~OsdControl()
{
    m_server.close();
    if (!m_path.isEmpty())
        QLocalServer::removeServer(m_path);
}

bool OsdControl::listen(const QString &dir, QString *error)
{
    if (!QDir().mkpath(dir)) {
        if (error != nullptr)
            *error = QStringLiteral("cannot create %1").arg(dir);
        return false;
    }
    QFile::setPermissions(dir, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
    m_path = dir + QStringLiteral("/osd.sock");
    // A socket file left by a crashed pill is replaced; a live pill would
    // answer, and two pills on one session are the plugin guard's job.
    QLocalServer::removeServer(m_path);
    if (!m_server.listen(m_path)) {
        if (error != nullptr)
            *error = QStringLiteral("%1: %2").arg(m_path, m_server.errorString());
        m_path.clear();
        return false;
    }
    clearNotice(dir);
    qCInfo(lcOsdControl) << "control socket at" << m_path;
    return true;
}

void OsdControl::onConnection()
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
                reply = m_model->applyCommand(doc.object());
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

bool OsdControl::writeNotice(const QString &dir, const QString &host, const QString &notice)
{
    if (!QDir().mkpath(dir))
        return false;
    QSaveFile file(dir + QStringLiteral("/osd.status.json"));
    if (!file.open(QIODevice::WriteOnly))
        return false;
    const QJsonObject object{{QStringLiteral("host"), host}, {QStringLiteral("notice"), notice}};
    file.write(QJsonDocument(object).toJson(QJsonDocument::Compact) + '\n');
    return file.commit();
}

void OsdControl::clearNotice(const QString &dir)
{
    QFile::remove(dir + QStringLiteral("/osd.status.json"));
}

}  // namespace dettivo
