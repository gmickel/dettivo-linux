// The `dettivo` command line, run from the app for what has one
// implementation there (ADR 0033): `dettivo mcp config` writes a host's
// MCP entry and `dettivo doctor` composes the report, so the Agents and
// Diagnostics routes run the same code the terminal does instead of a
// second copy. The binary beside the app is preferred, then PATH; the
// tests hand in a fake.
#pragma once

#include <QObject>
#include <QString>
#include <QStringList>

#include <functional>

namespace dettivo {

class CliRunner : public QObject {
    Q_OBJECT

public:
    /// The outcome of one run: the exit code, standard output and error.
    using Done = std::function<void(int exitCode, const QString &out, const QString &err)>;

    using QObject::QObject;

    /// Runs `dettivo <args>`; `done` runs on the GUI thread.
    virtual void run(const QStringList &args, Done done) = 0;
    /// The program that runs, for the line the routes show.
    virtual QString program() const = 0;
};

/// The real thing: QProcess over the located binary.
class ProcessRunner : public CliRunner {
    Q_OBJECT

public:
    explicit ProcessRunner(QObject *parent = nullptr);

    /// Beside the running binary, then `PATH`; empty when neither has it.
    static QString locate();

    void run(const QStringList &args, Done done) override;
    QString program() const override { return m_program; }

private:
    QString m_program;
};

}  // namespace dettivo
