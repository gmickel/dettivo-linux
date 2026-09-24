#include "hotkeys_setup_model.h"

#include "config_binding.h"
#include "status_format.h"

#include <QClipboard>
#include <QGuiApplication>
#include <QJsonArray>
#include <QProcessEnvironment>

namespace dettivo {

HotkeysSetupModel::HotkeysSetupModel(DaemonLink *link, ConfigBinding *config, QObject *parent)
    : QObject(parent), m_link(link), m_config(config)
{
    if (m_link != nullptr)
        connect(m_link, &DaemonLink::connectedChanged, this, &HotkeysSetupModel::onConnected);
    if (m_config != nullptr)
        connect(m_config, &ConfigBinding::changed, this, [this]() {
            if (m_active)
                refresh();
        });
}

void HotkeysSetupModel::start()
{
    setActive(true);
}

void HotkeysSetupModel::setActive(bool active)
{
    if (m_active == active)
        return;
    m_active = active;
    if (m_link != nullptr && m_link->connected())
        onConnected(true);
}

void HotkeysSetupModel::onConnected(bool connected)
{
    if (connected && m_active)
        refresh();
}

void HotkeysSetupModel::refresh()
{
    if (m_link == nullptr || !m_link->connected())
        return;
    m_link->call(QStringLiteral("hotkeys.snippet"), QJsonObject(),
                 [this](const QJsonObject &result, const QJsonObject &error) { applySnippet(result, error); });
    m_link->call(QStringLiteral("hotkeys.setup"), QJsonObject{{QStringLiteral("write"), false}},
                 [this](const QJsonObject &result, const QJsonObject &error) {
                     if (error.isEmpty())
                         applySetup(result);
                 });
    m_link->call(QStringLiteral("hotkeys.status"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty())
            applyStatus(result);
    });
}

void HotkeysSetupModel::applySnippet(const QJsonObject &result, const QJsonObject &error)
{
    const QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    if (!error.isEmpty()) {
        m_supported = false;
        m_snippetText.clear();
        m_outcome = error.value(QStringLiteral("message")).toString();
    } else {
        m_supported = true;
        m_compositor = result.value(QStringLiteral("compositor")).toString();
        m_snippetText = result.value(QStringLiteral("text")).toString();
        m_snippetPath = format::homePath(result.value(QStringLiteral("path")).toString(), env);
    }
    emit changed();
}

void HotkeysSetupModel::applySetup(const QJsonObject &result)
{
    const QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    m_written = result.value(QStringLiteral("written")).toBool(false);
    m_sourced = result.value(QStringLiteral("sourced")).toBool(false);
    m_includeLine = result.value(QStringLiteral("include_line")).toString();
    const QJsonObject main = result.value(QStringLiteral("main_config")).toObject();
    m_mainConfigPath = format::homePath(main.value(QStringLiteral("path")).toString(), env);
    if (result.contains(QStringLiteral("path")))
        m_snippetPath = format::homePath(result.value(QStringLiteral("path")).toString(), env);
    emit changed();
}

void HotkeysSetupModel::applyStatus(const QJsonObject &result)
{
    m_daemonBackend = result.value(QStringLiteral("backend")).toString();
    emit changed();
}

QString HotkeysSetupModel::statusLine() const
{
    if (!m_supported)
        return tr("No binding snippet for this desktop; the keys go through the portal (backend %1).").arg(m_daemonBackend.isEmpty() ? tr("none") : m_daemonBackend);
    if (m_written && m_sourced)
        return tr("Written and sourced from %1.").arg(m_mainConfigPath);
    if (m_written)
        return tr("Written. Add %1 to %2 and reload.").arg(m_includeLine, m_mainConfigPath);
    return m_snippetPath.endsWith(QStringLiteral(".lua"))
        ? tr("Not written yet. Setup writes the bindings, includes them in Hyprland and reloads it.")
        : tr("Not written yet. Rewrite snippet writes this file.");
}

void HotkeysSetupModel::rewrite()
{
    if (m_link == nullptr || !m_link->connected())
        return;
    m_link->call(QStringLiteral("hotkeys.setup"), QJsonObject{{QStringLiteral("write"), true}},
                 [this](const QJsonObject &result, const QJsonObject &error) {
                     if (!error.isEmpty()) {
                         m_outcome = error.value(QStringLiteral("message")).toString();
                         emit changed();
                         return;
                     }
                     m_outcome = m_snippetPath.endsWith(QStringLiteral(".lua"))
                         ? tr("Shortcuts activated in Hyprland.") : tr("Snippet written.");
                     applySetup(result);
                 });
}

void HotkeysSetupModel::copyIncludeLine() const
{
    if (QClipboard *clipboard = QGuiApplication::clipboard())
        clipboard->setText(m_includeLine);
}

void HotkeysSetupModel::applySample()
{
    m_supported = true;
    m_compositor = QStringLiteral("Hyprland");
    m_snippetPath = QStringLiteral("~/.config/hypr/bindings/dettivo.conf");
    m_snippetText = QStringLiteral(
        "bind  = , F9, exec, dettivo dictation start\n"
        "bindr = , F9, exec, dettivo dictation stop\n"
        "bind  = SUPER CTRL, X, exec, dettivo dictation toggle\n"
        "bind  = SUPER CTRL SHIFT, X, exec, dettivo dictation reinsert-last\n"
        "bind  = SUPER CTRL, Escape, exec, dettivo dictation cancel\n");
    m_includeLine = QStringLiteral("source = ~/.config/hypr/bindings/dettivo.conf");
    m_mainConfigPath = QStringLiteral("~/.config/hypr/hyprland.conf");
    m_written = true;
    m_sourced = true;
    m_daemonBackend = QStringLiteral("none");
    emit changed();
}

}  // namespace dettivo
