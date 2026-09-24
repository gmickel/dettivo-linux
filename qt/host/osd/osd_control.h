// The pill's control socket: `osd.sock` beside the daemon socket, one JSON
// line in (`show`, `hide`, `status`) and one line out, for `dettivo osd`
// and the QA drives. The disabled notice lives beside it as
// `osd.status.json` so `dettivo doctor` can name why no pill runs.
#pragma once

#include <QLocalServer>
#include <QObject>
#include <QString>

namespace dettivo {

class OsdModel;

class OsdControl : public QObject {
    Q_OBJECT

public:
    explicit OsdControl(OsdModel *model, QObject *parent = nullptr);
    ~OsdControl() override;

    /// Listens on `<dir>/osd.sock`, replacing a stale file; `error` says
    /// why when it cannot.
    bool listen(const QString &dir, QString *error);
    QString socketPath() const { return m_path; }

    /// Writes `<dir>/osd.status.json` for a pill that exits disabled.
    static bool writeNotice(const QString &dir, const QString &host, const QString &notice);
    /// Removes the notice once a pill runs.
    static void clearNotice(const QString &dir);

private:
    void onConnection();

    OsdModel *m_model;
    QLocalServer m_server;
    QString m_path;
};

}  // namespace dettivo
