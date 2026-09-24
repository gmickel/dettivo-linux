#include "first_run_model.h"
#include "config_binding.h"
#include "first_run_models.h"
#include "status_format.h"

#include <QDateTime>
#include <QDesktopServices>
#include <QJsonArray>
#include <QLoggingCategory>
#include <QProcessEnvironment>
#include <QUrl>

namespace dettivo {

Q_LOGGING_CATEGORY(lcFirstRun, "dettivo.app.firstrun")

namespace {

QString nowIso() { return QDateTime::currentDateTimeUtc().toString(Qt::ISODate); }

QString backendLabel(QString backend) { return backend.replace(QLatin1Char('_'), QLatin1Char(' ')); }

}  // namespace

FirstRunModel::FirstRunModel(DaemonLink *link, ConfigBinding *config, QObject *parent)
    : QObject(parent), m_link(link), m_config(config), m_models(new FirstRunModels(link, this))
{
    m_pressPoll.setInterval(1000);
    connect(&m_pressPoll, &QTimer::timeout, this, &FirstRunModel::pollPress);
    if (m_link != nullptr) {
        connect(m_link, &DaemonLink::notification, this, &FirstRunModel::handleNotification);
        connect(m_link, &DaemonLink::connectedChanged, this, &FirstRunModel::onConnected);
    }
    if (m_config != nullptr)
        connect(m_config, &ConfigBinding::changed, this, &FirstRunModel::readKeys);
}

QStringList FirstRunModel::steps()
{
    return {QStringLiteral("keys"), QStringLiteral("models"), QStringLiteral("try")};
}

QString FirstRunModel::compositorLabel(const QString &platformCompositor)
{
    const QString lower = platformCompositor.toLower();
    if (lower == QStringLiteral("hyprland"))
        return QStringLiteral("Hyprland");
    if (lower == QStringLiteral("sway"))
        return QStringLiteral("Sway");
    if (lower == QStringLiteral("niri"))
        return QStringLiteral("Niri");
    return platformCompositor;
}

QObject *FirstRunModel::models() const
{
    return m_models;
}

void FirstRunModel::start()
{
    if (m_link != nullptr && m_link->connected())
        onConnected(true);
}

void FirstRunModel::markComplete()
{
    m_pressPoll.stop();
    m_forcedOpen = false;
    m_completed = true;
    m_decided = true;
    m_required = false;
    emit decisionChanged();
}

void FirstRunModel::openAt(const QString &step)
{
    const bool wasActive = active();
    m_forcedOpen = true;
    if (!wasActive && m_link != nullptr && m_link->connected()) {
        m_connectionProcessed = true;
        activate(false);
    }
    setStep(steps().contains(step) ? step : (steps().contains(m_step) ? m_step : QStringLiteral("keys")));
}

void FirstRunModel::leave()
{
    m_forcedOpen = false;
    if (!active())
        m_pressPoll.stop();
}

QVariantList FirstRunModel::bindings() const
{
    auto row = [](const QString &action, const QString &chord, const QString &hint, const QString &runs) {
        return QVariantMap{{QStringLiteral("action"), action},
                           {QStringLiteral("keys"), chord.split(QLatin1Char('+'), Qt::SkipEmptyParts)},
                           {QStringLiteral("hint"), hint},
                           {QStringLiteral("runs"), runs}};
    };
    const QString hold = m_hold.isEmpty() ? QStringLiteral("F9") : m_hold;
    const QString toggle = m_toggle.isEmpty() ? QStringLiteral("Super+Ctrl+X") : m_toggle;
    const QString cancel = m_cancel.isEmpty() ? QStringLiteral("Super+Ctrl+Esc") : m_cancel;
    const QString reinsert = m_reinsert.isEmpty() ? QStringLiteral("Super+Ctrl+Shift+X") : m_reinsert;
    return {row(tr("Hold to talk"), hold, tr("press starts, release inserts"), tr("dettivo dictation start / stop")),
            row(tr("Toggle dictation"), toggle, QString(), tr("dettivo dictation toggle")),
            row(tr("Cancel"), cancel, tr("while listening"), tr("dettivo dictation cancel")),
            row(tr("Insert again"), reinsert, tr("the last transcript"), tr("dettivo dictation reinsert-last"))};
}

void FirstRunModel::readKeys()
{
    if (!active())
        return;
    if (m_config != nullptr) {
        m_hold = format::chord(m_config->text(QStringLiteral("hotkeys.hold")));
        m_toggle = format::chord(m_config->text(QStringLiteral("hotkeys.toggle")));
        m_cancel = format::chord(m_config->text(QStringLiteral("hotkeys.cancel")));
        m_reinsert = format::chord(m_config->text(QStringLiteral("hotkeys.reinsert")));
    }
    if (m_link != nullptr && m_link->connected()) {
        m_link->call(QStringLiteral("config.path"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
            if (error.isEmpty()) {
                m_configPath = format::homePath(result.value(QStringLiteral("config")).toString(), QProcessEnvironment::systemEnvironment());
                emit keysChanged();
            }
        });
    }
    emit keysChanged();
}

void FirstRunModel::readSnippet(bool includeSetup)
{
    m_link->call(QStringLiteral("hotkeys.snippet"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        const QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
        if (!error.isEmpty()) {
            m_snippetSupported = false;
            m_snippetText.clear();
            emit keysChanged();
            return;
        }
        m_snippetSupported = true;
        m_snippetText = result.value(QStringLiteral("text")).toString();
        m_snippetPath = format::homePath(result.value(QStringLiteral("path")).toString(), env);
        m_includeLine = result.value(QStringLiteral("include_line")).toString();
        emit keysChanged();
    });
    if (includeSetup)
        m_link->call(QStringLiteral("hotkeys.setup"), QJsonObject{{QStringLiteral("write"), false}}, [this](const QJsonObject &result, const QJsonObject &error) {
            if (error.isEmpty())
                applySetup(result);
        });
}

void FirstRunModel::applySetup(const QJsonObject &result)
{
    m_snippetWritten = result.value(QStringLiteral("written")).toBool(false);
    m_snippetSourced = result.value(QStringLiteral("sourced")).toBool(false);
    m_mainConfigPath = format::homePath(result.value(QStringLiteral("main_config")).toObject().value(QStringLiteral("path")).toString(), QProcessEnvironment::systemEnvironment());
    emit keysChanged();
}

void FirstRunModel::readHotkeysStatus()
{
    m_link->call(QStringLiteral("hotkeys.status"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty())
            applyHotkeysStatus(result);
    });
}

void FirstRunModel::applyHotkeysStatus(const QJsonObject &result)
{
    const QString backend = result.value(QStringLiteral("backend")).toString();
    const QJsonObject portal = result.value(QStringLiteral("portal")).toObject();
    m_bound = !result.value(QStringLiteral("bound")).toArray().isEmpty();
    if (m_lastPressAt.isEmpty())
        m_lastPressAt = result.value(QStringLiteral("last_press_at")).toString();
    if (backend != QStringLiteral("none") && m_bound)
        m_portalLine = tr("Bound through the desktop portal (%1). Press the key to check.").arg(backend);
    else if (portal.value(QStringLiteral("available")).toBool(false))
        m_portalLine = tr("The desktop portal offers global shortcuts; the daemon binds them when it starts with a portal backend.");
    else
        m_portalLine = tr("No compositor snippet and no portal here: %1").arg(portal.value(QStringLiteral("reason")).toString());
    m_keysError = result.value(QStringLiteral("error")).toString();
    emit keysChanged();
}

void FirstRunModel::readDevices()
{
    m_link->call(QStringLiteral("audio.devices"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty())
            return;
        QStringList names;
        const QString wanted = result.value(QStringLiteral("pinned")).toString().isEmpty()
                                   ? result.value(QStringLiteral("default_source")).toString()
                                   : result.value(QStringLiteral("pinned")).toString();
        m_inputName.clear();
        for (const QJsonValue &d : result.value(QStringLiteral("devices")).toArray()) {
            const QJsonObject device = d.toObject();
            names.append(device.value(QStringLiteral("description")).toString());
            if (device.value(QStringLiteral("name")).toString() == wanted)
                m_inputName = device.value(QStringLiteral("description")).toString();
        }
        m_micAvailable = !names.isEmpty();
        m_micLine = m_micAvailable ? QString()
                                   : (result.value(QStringLiteral("pipewire")).toBool(false)
                                          ? tr("No microphone: PipeWire lists no input device. You can finish anyway and plug one in later.")
                                          : tr("No microphone: PipeWire is not running. You can finish anyway."));
        if (m_micAvailable && m_inputName.isEmpty())
            m_inputName = names.first();
        emit tryChanged();
        emit pressChanged();
    });
}

void FirstRunModel::readSelectionLine()
{
    m_link->call(QStringLiteral("speech.selection.get"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty())
            return;
        const QJsonObject dictation = result.value(QStringLiteral("dictation")).toObject();
        m_engineLine = format::modelName(dictation.value(QStringLiteral("model_id")).toString());
        emit tryChanged();
    });
}

void FirstRunModel::pollPress()
{
    if (!active() || m_link == nullptr || !m_link->connected() || m_pressed || m_step != QStringLiteral("keys"))
        return;
    m_link->call(QStringLiteral("hotkeys.status"), QJsonObject(), [this](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty())
            return;
        const QString at = result.value(QStringLiteral("last_press_at")).toString();
        if (!at.isEmpty() && at != m_lastPressAt) {
            m_lastPressAt = at;
            m_pressed = true;
            m_pressLine = tr("%1 pressed · reached the daemon").arg(m_hold.isEmpty() ? QStringLiteral("F9") : m_hold);
            emit pressChanged();
        }
    });
}

void FirstRunModel::handleNotification(const QString &topic, const QJsonObject &payload)
{
    if (topic != QStringLiteral("dictation.state"))
        return;
    const QString state = payload.value(QStringLiteral("state")).toString();
    m_dictationState = state == QStringLiteral("cancelled") || state == QStringLiteral("failed") ? QStringLiteral("idle") : state;
    if (state == QStringLiteral("recording") && !m_pressed) {
        m_pressed = true;
        m_pressLine = tr("%1 held · listening · %2").arg(m_hold.isEmpty() ? QStringLiteral("F9") : m_hold, m_inputName.isEmpty() ? tr("default input") : m_inputName);
        emit pressChanged();
    }
    if (state == QStringLiteral("idle") && payload.value(QStringLiteral("previous_state")).toString() == QStringLiteral("inserting")) {
        const QJsonObject insertion = payload.value(QStringLiteral("insertion")).toObject();
        const QJsonObject timings = payload.value(QStringLiteral("timings")).toObject();
        const QString outcome = insertion.value(QStringLiteral("outcome")).toString();
        m_resultKnown = true;
        m_resultOutcome = outcome;
        m_insertedVia = outcome == QStringLiteral("inserted") ? backendLabel(insertion.value(QStringLiteral("backend")).toString())
                                                              : (outcome == QStringLiteral("copied_to_clipboard") ? tr("clipboard") : tr("not inserted"));
        const double total = timings.value(QStringLiteral("capture_ms")).toDouble() + timings.value(QStringLiteral("transcribe_ms")).toDouble()
            + timings.value(QStringLiteral("insert_ms")).toDouble();
        m_stopToInsert = tr("%1 s").arg(QString::number(total / 1000.0, 'f', 1));
        const QString mode = payload.value(QStringLiteral("mode")).toString();
        m_modeLine = mode == QStringLiteral("enhanced") ? tr("Enhanced") : (mode.contains(QStringLiteral("polish")) ? tr("Polish") : tr("Raw"));
        m_resultReason = outcome == QStringLiteral("inserted") ? QString() : insertion.value(QStringLiteral("reason")).toString();
    }
    emit tryChanged();
}

void FirstRunModel::writeSnippet()
{
    writeSnippetAndContinue(false);
}

void FirstRunModel::writeSnippetAndContinue(bool advance)
{
    if (m_setupPending || !m_snippetSupported)
        return;
    if (m_link == nullptr || !m_link->connected()) {
        m_keysError = tr("Connect to Dettivo, then retry shortcut setup.");
        emit keysChanged();
        return;
    }
    m_setupPending = true;
    m_keysError.clear();
    emit keysChanged();
    m_link->call(QStringLiteral("hotkeys.setup"), QJsonObject{{QStringLiteral("write"), true}}, [this, advance](const QJsonObject &result, const QJsonObject &error) {
        m_setupPending = false;
        if (!error.isEmpty()) {
            m_snippetSourced = false;
            m_keysError = error.value(QStringLiteral("message")).toString();
            emit keysChanged();
            return;
        }
        m_snippetWritten = result.value(QStringLiteral("written")).toBool(false);
        m_snippetSourced = result.value(QStringLiteral("sourced")).toBool(false);
        const bool ready = m_snippetWritten && (!m_snippetPath.endsWith(QStringLiteral(".lua")) || m_snippetSourced);
        m_keysError = ready ? QString() : tr("Shortcuts are not activated. Retry shortcut setup or skip for now.");
        emit keysChanged();
        if (advance && ready && m_step == QStringLiteral("keys"))
            setStep(QStringLiteral("models"));
    });
}

void FirstRunModel::setStep(const QString &step)
{
    if (step == m_step)
        return;
    if (m_step == QStringLiteral("try") && m_allowanceArmed)
        setAllowance(false);
    m_step = step;
    if (active() && step == QStringLiteral("keys") && m_link != nullptr && m_link->connected())
        m_pressPoll.start();
    else
        m_pressPoll.stop();
    emit stepChanged();
}

void FirstRunModel::next()
{
    if (m_setupPending)
        return;
    const int i = stepIndex();
    if (i < 0 || i + 1 >= steps().size())
        return;
    if (m_step == QStringLiteral("keys") && m_snippetSupported
        && (!m_snippetWritten || (m_snippetPath.endsWith(QStringLiteral(".lua")) && !m_snippetSourced))) {
        writeSnippetAndContinue(true);
        return;
    }
    setStep(steps().at(i + 1));
}

void FirstRunModel::back()
{
    const int i = stepIndex();
    if (i > 0)
        setStep(steps().at(i - 1));
}

void FirstRunModel::skip()
{
    const int i = stepIndex();
    if (i >= 0 && i + 1 < steps().size())
        setStep(steps().at(i + 1));
}

void FirstRunModel::setAllowance(bool enabled)
{
    if (m_link == nullptr || !m_link->connected()) {
        m_allowanceArmed = false;
        emit tryChanged();
        return;
    }
    m_link->call(QStringLiteral("insert.allow_self_target"), QJsonObject{{QStringLiteral("enabled"), enabled}},
                 [this](const QJsonObject &result, const QJsonObject &error) {
                     m_allowanceArmed = error.isEmpty() && result.value(QStringLiteral("enabled")).toBool(false);
                     if (!error.isEmpty())
                         qCWarning(lcFirstRun) << "insert.allow_self_target refused:" << error.value(QStringLiteral("message")).toString();
                     emit tryChanged();
                 });
}

void FirstRunModel::finish()
{
    if (m_allowanceArmed)
        setAllowance(false);
    m_forcedOpen = false;
    m_completed = true;
    m_required = false;
    m_decided = true;
    m_pressPoll.stop();
    emit decisionChanged();
    emit completed(nowIso());
}

void FirstRunModel::openConfig()
{
    if (m_link == nullptr || !m_link->connected())
        return;
    m_link->call(QStringLiteral("config.path"), QJsonObject(), [](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty())
            QDesktopServices::openUrl(QUrl::fromLocalFile(result.value(QStringLiteral("config")).toString()));
    });
}

void FirstRunModel::applySample(const QString &step)
{
    m_decided = true;
    m_required = true;
    m_compositor = QStringLiteral("Hyprland");
    m_platformCompositor = QStringLiteral("Hyprland");
    m_hold = QStringLiteral("F9");
    m_toggle = QStringLiteral("Super+Ctrl+X");
    m_cancel = QStringLiteral("Super+Ctrl+Esc");
    m_reinsert = QStringLiteral("Super+Ctrl+Shift+X");
    m_inputName = QStringLiteral("Arctis Nova");
    m_configPath = QStringLiteral("~/.config/dettivo/config.toml");
    m_snippetSupported = true;
    m_snippetPath = QStringLiteral("~/.config/hypr/dettivo.conf");
    m_includeLine = QStringLiteral("source = ~/.config/hypr/dettivo.conf");
    m_mainConfigPath = QStringLiteral("~/.config/hypr/hyprland.conf");
    m_snippetText = QStringLiteral("unbind = , F9\nunbind = SUPER CTRL, X\nbind = , F9, exec, dettivo --quiet dictation start\n"
                                   "bindr = , F9, exec, dettivo --quiet dictation stop\nbind = SUPER CTRL, X, exec, dettivo --quiet dictation toggle\n"
                                   "bind = SUPER CTRL, Escape, exec, dettivo --quiet dictation cancel\n"
                                   "bind = SUPER CTRL SHIFT, X, exec, dettivo --quiet dictation reinsert-last\n");
    m_snippetWritten = true;
    m_snippetSourced = true;
    m_pressed = true;
    m_pressLine = QStringLiteral("F9 held · listening · Arctis Nova");
    m_models->setTier(QStringLiteral("vulkan"));
    m_models->applyStatus(sampleModels());
    m_models->applyProviders(QJsonArray(), QStringList());
    m_models->applyLlmProvider(QStringLiteral("local"));
    m_dictationState = QStringLiteral("recording");
    m_engineLine = QStringLiteral("Parakeet v3");
    m_resultKnown = true;
    m_resultOutcome = QStringLiteral("inserted");
    m_insertedVia = QStringLiteral("virtual keyboard");
    m_stopToInsert = QStringLiteral("0.8 s");
    m_modeLine = QStringLiteral("Enhanced · Qwen3 4B");
    m_sampleText = QStringLiteral("This is the first thing I have said to Dettivo on Linux, and it landed here.");
    m_micAvailable = true;
    setStep(step);
    emit decisionChanged();
    emit keysChanged();
    emit pressChanged();
    emit tryChanged();
}

QJsonObject FirstRunModel::sampleModels()
{
    auto model = [](const char *provider, const char *id, const char *name, const char *rec, double size, const char *readiness, double done, bool selected, const char *lang) {
        return QJsonObject{{QStringLiteral("provider"), QLatin1String(provider)},
                           {QStringLiteral("id"), QLatin1String(id)},
                           {QStringLiteral("display_name"), QLatin1String(name)},
                           {QStringLiteral("kind"), QStringLiteral("stt")},
                           {QStringLiteral("recommended_for"), QString::fromUtf8(rec)},
                           {QStringLiteral("size_bytes"), size},
                           {QStringLiteral("readiness"), QLatin1String(readiness)},
                           {QStringLiteral("bytes_done"), done},
                           {QStringLiteral("bytes_total"), size},
                           {QStringLiteral("is_selected"), selected},
                           {QStringLiteral("is_default"), true},
                           {QStringLiteral("available"), true},
                           {QStringLiteral("languages"), QJsonArray{QLatin1String(lang)}},
                           {QStringLiteral("error"), QJsonValue::Null},
                           {QStringLiteral("path"), QJsonValue::Null},
                           {QStringLiteral("license"), QStringLiteral("MIT")}};
    };
    return {{QStringLiteral("models_dir"), QStringLiteral("~/.local/share/dettivo/models")},
            {QStringLiteral("models"),
             QJsonArray{model("parakeet", "parakeet-v3", "Parakeet TDT 0.6B v3", "25 European languages · word timestamps · fastest on GPU", 640e6,
                              "downloading", 397e6, true, "multilingual"),
                        model("whisper", "large-v3-turbo", "Large v3 Turbo", "99 languages · vocabulary prompts · the Mac default", 875e6, "missing", 0,
                              false, "multilingual"),
                        model("whisper", "small.en", "Small (English)", "English · light enough for CPU-only laptops", 466e6, "missing", 0, false,
                              "en")}}};
}

}  // namespace dettivo
