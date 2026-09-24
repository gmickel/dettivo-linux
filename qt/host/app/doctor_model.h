// Diagnostics (ADR 0033): `dettivo doctor` on screen. The route runs the
// command the terminal runs, shows its report line by line with the exit
// code as the verdict, and copies it for a bug report; nothing here is a
// second diagnosis.
#pragma once

#include "cli_runner.h"

#include <QObject>
#include <QString>
#include <QStringList>

namespace dettivo {

class DoctorModel : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString report READ report NOTIFY reportChanged)
    Q_PROPERTY(QStringList lines READ lines NOTIFY reportChanged)
    Q_PROPERTY(QString verdict READ verdict NOTIFY reportChanged)
    Q_PROPERTY(bool healthy READ healthy NOTIFY reportChanged)
    Q_PROPERTY(bool running READ running NOTIFY runningChanged)
    Q_PROPERTY(QString ranAt READ ranAt NOTIFY reportChanged)
    Q_PROPERTY(QString command READ command CONSTANT)

public:
    explicit DoctorModel(CliRunner *runner, QObject *parent = nullptr);

    /// Runs the report (again).
    Q_INVOKABLE void run();
    /// Puts the report on the clipboard.
    Q_INVOKABLE void copy() const;

    QString report() const { return m_report; }
    QStringList lines() const { return m_report.split(QLatin1Char('\n'), Qt::SkipEmptyParts); }
    QString verdict() const;
    bool healthy() const { return m_exitCode == 0; }
    bool running() const { return m_running; }
    QString ranAt() const { return m_ranAt; }
    QString command() const;

    /// The baseline's report with no daemon (the renders and the tests).
    void applySample();
    /// A finished run (tests).
    void applyReport(int exitCode, const QString &text);

signals:
    void reportChanged();
    void runningChanged();

private:
    CliRunner *m_runner;
    QString m_report;
    QString m_ranAt;
    int m_exitCode = -1;
    bool m_running = false;
};

}  // namespace dettivo
