#include "cli_runner.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QProcess>
#include <QStandardPaths>

namespace dettivo {

ProcessRunner::ProcessRunner(QObject *parent) : CliRunner(parent), m_program(locate()) {}

QString ProcessRunner::locate()
{
    const QDir beside(QCoreApplication::applicationDirPath());
    for (const QString &candidate : {beside.filePath(QStringLiteral("dettivo")),
                                     beside.filePath(QStringLiteral("../dettivo/dettivo")),
                                     beside.filePath(QStringLiteral("../../../target/debug/dettivo")),
                                     beside.filePath(QStringLiteral("../../../target/release/dettivo"))}) {
        const QFileInfo info(candidate);
        if (info.isExecutable() && info.isFile())
            return info.canonicalFilePath();
    }
    return QStandardPaths::findExecutable(QStringLiteral("dettivo"));
}

void ProcessRunner::run(const QStringList &args, Done done)
{
    if (m_program.isEmpty()) {
        if (done)
            done(127, QString(), QStringLiteral("dettivo is not installed beside the app or on PATH"));
        return;
    }
    auto *process = new QProcess(this);
    process->setProgram(m_program);
    process->setArguments(args);
    // The app has no project directory: a host file the command line
    // resolves against the working directory (Claude Code's .mcp.json)
    // lands under home, where a launcher would have started the app.
    process->setWorkingDirectory(QDir::homePath());
    connect(process, &QProcess::finished, this, [process, done](int exitCode, QProcess::ExitStatus status) {
        const QString out = QString::fromUtf8(process->readAllStandardOutput());
        const QString err = QString::fromUtf8(process->readAllStandardError());
        process->deleteLater();
        if (done)
            done(status == QProcess::NormalExit ? exitCode : 128, out, err);
    });
    connect(process, &QProcess::errorOccurred, this, [process, done](QProcess::ProcessError error) {
        if (error != QProcess::FailedToStart)
            return;
        process->deleteLater();
        if (done)
            done(127, QString(), process->errorString());
    });
    process->start();
}

}  // namespace dettivo
