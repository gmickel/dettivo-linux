#include "doctor_model.h"

#include <QClipboard>
#include <QDateTime>
#include <QGuiApplication>

namespace dettivo {

DoctorModel::DoctorModel(CliRunner *runner, QObject *parent) : QObject(parent), m_runner(runner) {}

QString DoctorModel::command() const
{
    return QStringLiteral("dettivo doctor");
}

QString DoctorModel::verdict() const
{
    if (m_running)
        return tr("Running dettivo doctor…");
    if (m_exitCode < 0)
        return tr("Not run yet.");
    if (m_exitCode == 0)
        return tr("Everything answers.");
    if (m_exitCode == 127)
        return tr("dettivo is not installed beside the app or on PATH.");
    return tr("Doctor found problems (see the report).");
}

void DoctorModel::run()
{
    if (m_runner == nullptr || m_running)
        return;
    m_running = true;
    emit runningChanged();
    m_runner->run({QStringLiteral("doctor")}, [this](int exitCode, const QString &out, const QString &err) {
        m_running = false;
        emit runningChanged();
        // The report prints whatever the exit code is; a refusal to run
        // at all (no binary, no daemon answer) is the error text.
        applyReport(exitCode, out.isEmpty() ? err : out);
    });
}

void DoctorModel::applyReport(int exitCode, const QString &text)
{
    m_exitCode = exitCode;
    m_report = text.trimmed();
    m_ranAt = QDateTime::currentDateTime().toString(QStringLiteral("HH:mm:ss"));
    emit reportChanged();
}

void DoctorModel::copy() const
{
    if (QClipboard *clipboard = QGuiApplication::clipboard())
        clipboard->setText(m_report + QLatin1Char('\n'));
}

void DoctorModel::applySample()
{
    m_exitCode = 0;
    m_running = false;
    m_ranAt = QStringLiteral("14:32:08");
    m_report = QStringLiteral(
        "socket    $XDG_RUNTIME_DIR/dettivo/dettivo.sock (present)\n"
        "service   dettivod.socket enabled/active, dettivod.service active\n"
        "daemon    reachable, version 1.0.0 build 2026.09.05, health ok=true uptime 5124s\n"
        "auth      peer\n"
        "platform  Hyprland on wayland (linux), insertion virtual_keyboard, gpu vulkan\n"
        "insert    virtual_keyboard available; libei available; ydotool unavailable (no ydotoold); clipboard available\n"
        "hotkeys   backend none; portal unavailable (no GlobalShortcuts on Hyprland); evdev available (input group)\n"
        "snippet   ~/.config/hypr/bindings/dettivo.conf written, sourced from ~/.config/hypr/hyprland.conf\n"
        "audio     PipeWire, default source alsa_input.usb-SteelSeries_Arctis_Nova, 3 devices, pinned none (following the default)\n"
        "config    ok: ~/.config/dettivo/config.toml\n"
        "engine    dettivo-engine-parakeet running parakeet-v3 on vulkan\n"
        "engine    dettivo-engine-whisper found, not running\n"
        "models    4 ready under ~/.local/share/dettivo/models, 0 quarantined; selected parakeet/parakeet-v3 ready\n"
        "llm       provider local; local available (llm/qwen3-4b-instruct-2507 ready); dettivo-engine-llm found, not running\n"
        "history   412 items (398 with audio) in ~/.local/share/dettivo/dettivo.db (18240 KB), schema v4 (0004-timings)\n"
        "osd       layer_shell host at top on focused, hidden\n"
        "env       no DETTIVO_* overrides");
    emit reportChanged();
    emit runningChanged();
}

}  // namespace dettivo
