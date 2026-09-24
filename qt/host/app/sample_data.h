// The baselines' facts with no daemon (fn-17 R2, fn-22 R4): the status
// sentence, the instrument strip, the rows of Today, the four engines and
// the agent surfaces of home.png, and the three days of rows, the detail
// and the take of history.png, so the visual check renders the same
// structure the sheets fix.
#pragma once

#include <QDate>
#include <QJsonArray>
#include <QJsonObject>

namespace dettivo {

class AgentsModel;
class ConfigBinding;
class DoctorModel;
class EnginesModel;
class HotkeysSetupModel;
class ModelsTable;
class SettingsModel;
class HistoryActions;
class HistoryDetailModel;
class HistoryModel;
class HistoryPlayer;
class MeetingDetailModel;
class MeetingLiveModel;
class MeetingsActions;
class MeetingsModel;
class StatusModel;

namespace sample {

/// Wednesday 3 September on the artboards; sample renders never follow the wall clock.
QDate referenceDate();
/// The status facts (`hold`, `toggle`, `target`, `input`, ...).
QJsonObject status();
/// `transcripts.list` items over today, yesterday and the day before,
/// newest first; the newest day's six rows are Home's.
QJsonArray historyItems();
/// The id of the newest row, the one the detail shows.
QString detailId();
/// `transcripts.get` for the newest row: Enhanced, Raw, the take, the facts.
QJsonObject historyDetail();
/// `speech.providers.list` and `speech.selection.get` for the re-run dialog.
QJsonObject providers();
QJsonObject selection();
/// `speech.engines` rows and the selection that puts parakeet first.
QJsonArray engines();
/// `config.get` entries for the keys Home reads.
QJsonArray configEntries();

/// Applies all of it; a null pointer skips its part.
void apply(StatusModel *status, EnginesModel *enginesModel, HistoryModel *history, ConfigBinding *config,
           HistoryDetailModel *detail = nullptr, HistoryPlayer *player = nullptr, HistoryActions *actions = nullptr);

/// `config.get` entries for every key the settings routes edit, with the
/// artboards' values (settings-models.png, settings-hotkeys.png, agents.png).
QJsonArray settingsEntries();
/// Applies the settings routes' sample; a null pointer skips its part.
void applySettings(ConfigBinding *config, SettingsModel *settings, ModelsTable *models, HotkeysSetupModel *hotkeys,
                   AgentsModel *agents, DoctorModel *doctor);

/// The six `meetings.list` rows of meetings-list.png over this week and
/// last week, newest first.
QJsonArray meetingsItems();
/// The `speakers` of a listed meeting, for its swatches.
QJsonArray meetingSpeakers(const QString &id);
/// The analysed kickoff meeting-detail.png shows, and the notes-only row.
QString meetingDetailId();
QString meetingNotesOnlyId();
/// `meetings.get` for the kickoff: three speakers, seven segments, the
/// notes and the analysis.
QJsonObject meetingDetail();
/// The live meeting of meeting-live.png: the facts and its segments.
QJsonObject liveFacts();
QJsonArray liveSegments();
/// Applies the meetings sample for a `DETTIVO_E2E_MEETING_STATE`; a null
/// pointer skips its part.
void applyMeetings(const QString &state, MeetingsModel *meetings, MeetingLiveModel *live, MeetingDetailModel *detail,
                   MeetingsActions *actions);

}  // namespace sample
}  // namespace dettivo
