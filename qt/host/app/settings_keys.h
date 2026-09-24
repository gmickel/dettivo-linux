// Which config.toml keys each settings route edits (ADR 0033): one table
// the "This route writes" block, the round-trip drive and
// scripts/lint-settings-keys.sh all read, so a key the schema gains
// without an editor fails the lint by name. A key that no route edits is
// listed with the reason, never left out silently.
#pragma once

#include <QString>
#include <QStringList>

namespace dettivo::settings {

/// The eight sections in nav order.
QStringList sections();

/// The keys a section edits, in the order its rows show them.
QStringList keysFor(const QString &section);

/// Every key some section edits, once each.
QStringList editableKeys();

/// The reason a key has no editor, or empty when it has one.
QString reasonNotEditable(const QString &key);

/// The TOML table a key lives in (`engines.whisper` for
/// `engines.whisper.backend`) and its leaf name.
QString tableOf(const QString &key);
QString leafOf(const QString &key);

/// The environment variable that overrides a key, or empty.
QString environmentVariable(const QString &key);

}  // namespace dettivo::settings
