// qt-hosts/F12: a daemon that accepts the socket and never answers fails
// each pending callback exactly once at the request deadline, with the
// wait named and the uncertainty stated; the connection itself stays up.
#include "daemon_client.h"

#include <QCoreApplication>
#include <QJsonObject>
#include <QJsonDocument>
#include <QLocalServer>
#include <QLocalSocket>
#include <QTemporaryDir>
#include <QTest>
#include <QSignalSpy>

using namespace dettivo;

class DaemonClientTest : public QObject {
    Q_OBJECT

private slots:
    void aSilentDaemonFailsEachRequestOnceAtTheDeadline();
    void subscriptionReadinessWaitsForTheAcknowledgement();
};

void DaemonClientTest::subscriptionReadinessWaitsForTheAcknowledgement()
{
    QTemporaryDir dir;
    QLocalServer server;
    QVERIFY(server.listen(dir.filePath("daemon.sock")));
    DaemonClient client(server.fullServerName(), {});
    QSignalSpy ready(&client, &DaemonLink::subscriptionReady);
    client.subscribe({QStringLiteral("meeting.state")});
    client.start();
    QTRY_VERIFY(server.hasPendingConnections());
    auto *socket = server.nextPendingConnection();
    QTRY_VERIFY(socket->canReadLine());
    const auto request = QJsonDocument::fromJson(socket->readLine()).object();
    QCOMPARE(request.value("method").toString(), QStringLiteral("events.subscribe"));
    QCOMPARE(ready.size(), 0);
    socket->write(QJsonDocument(QJsonObject{{"jsonrpc", "2.0"}, {"id", request.value("id")}, {"result", QJsonObject()}}).toJson(QJsonDocument::Compact) + '\n');
    socket->flush();
    QTRY_COMPARE(ready.size(), 1);
    client.subscribe({QStringLiteral("meeting.state")});
    QTRY_VERIFY(socket->canReadLine());
    const auto refused = QJsonDocument::fromJson(socket->readLine()).object();
    socket->write(QJsonDocument(QJsonObject{{"jsonrpc", "2.0"}, {"id", refused.value("id")}, {"error", QJsonObject{{"message", "refused"}}}}).toJson(QJsonDocument::Compact) + '\n');
    socket->flush();
    client.call("system.ping", {}, {});
    QTRY_VERIFY(socket->canReadLine());
    QCOMPARE(ready.size(), 1);
    client.stop();
}

void DaemonClientTest::aSilentDaemonFailsEachRequestOnceAtTheDeadline()
{
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    const QString path = dir.filePath("dettivo.sock");
    QLocalServer server;
    QVERIFY2(server.listen(path), qPrintable(server.errorString()));
    QList<QLocalSocket *> accepted;
    connect(&server, &QLocalServer::newConnection, this, [&]() {
        while (QLocalSocket *socket = server.nextPendingConnection())
            accepted.append(socket);
    });

    DaemonClient client(path, QString());
    client.setRequestTimeoutMs(300);
    client.start();
    QTRY_VERIFY_WITH_TIMEOUT(client.connected(), 5000);

    int answered = 0;
    QJsonObject lastError;
    for (int i = 0; i < 2; ++i) {
        client.call(QStringLiteral("system.ping"), {}, [&](const QJsonObject &, const QJsonObject &error) {
            ++answered;
            lastError = error;
        });
    }
    QTRY_COMPARE_WITH_TIMEOUT(answered, 2, 5000);
    QVERIFY2(lastError.value(QStringLiteral("message")).toString().contains(QStringLiteral("no answer from the daemon within 300 ms")),
             qPrintable(lastError.value(QStringLiteral("message")).toString()));
    QVERIFY(lastError.value(QStringLiteral("message")).toString().contains(QStringLiteral("may still have taken effect")));
    QVERIFY(client.connected());
    QTest::qWait(700);
    QCOMPARE(answered, 2);

    // A late answer to an expired request completes nothing twice.
    QCOMPARE(accepted.size(), 1);
    accepted.first()->write("{\"jsonrpc\":\"2.0\",\"id\":\"1\",\"result\":{}}\n");
    accepted.first()->flush();
    QTest::qWait(200);
    QCOMPARE(answered, 2);
    client.stop();
}

QTEST_GUILESS_MAIN(DaemonClientTest)
#include "daemon_client_test.moc"
