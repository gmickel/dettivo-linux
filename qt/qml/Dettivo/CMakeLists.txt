# The shared `Dettivo` QML module (ADR 0001, ADR 0010): the theme backend,
# the Theme, Motion and Html singletons, the icon set and every reusable
# component. Installed to ${CMAKE_INSTALL_LIBDIR}/dettivo/qml/Dettivo so
# external consumers such as the Quickshell bar plugin can `import Dettivo`
# with import path /usr/lib/dettivo/qml.
#
# Backing library and plugin are one shared library (PLUGIN_TARGET equals
# the backing target): Qt does not allow a static backing library with a
# shared plugin, an external consumer needs a loadable .so, and the hosts
# link the same library so the build tree works without an install.
qt_add_library(Dettivo SHARED)

set(dettivo_singletons SettingsUi.qml Theme.qml Motion.qml style/StyleHelpers.qml Html.qml)
set_source_files_properties(${dettivo_singletons} PROPERTIES QT_QML_SINGLETON_TYPE TRUE)

set(dettivo_components
    components/BarGlyph.qml
    components/BarPanel.qml
    components/BarPanelFacts.qml
    components/BarPanelHeader.qml
    components/BarPanelRecent.qml
    components/Chip.qml
    components/FocusRing.qml
    components/Icon.qml
    components/KeyCap.qml
    components/KeyValueRow.qml
    components/LevelMeter.qml
    components/ListRow.qml
    components/Osd.qml
    components/OsdContent.qml
    components/OsdDot.qml
    components/OsdThread.qml
    components/SectionLabel.qml
    components/SegmentedControl.qml
    components/StatusDot.qml
    components/Waveform.qml
)

# The app's screens (fn-17, ADR 0020): the window, the sidebar, Home, the
# route scaffolds and the three first-run steps (ADR 0024); the host hands
# them the router and the models.
set(dettivo_app_files
    app/AppWindow.qml
    app/BindingRow.qml
    app/DaemonBanner.qml
    app/StateView.qml
    app/FocusFlickable.qml
    app/FirstRunFooter.qml
    app/HintSheet.qml
    app/HomeRoute.qml
    app/InstrumentStrip.qml
    app/KeySentence.qml
    app/KeysStep.qml
    app/ModelRow.qml
    app/ModelsStep.qml
    app/OnboardingRoute.qml
    app/PageHeader.qml
    app/RailRow.qml
    app/RightRail.qml
    app/RouteScaffold.qml
    app/SectionHeading.qml
    app/Sidebar.qml
    app/SidebarFooterRow.qml
    app/SidebarItem.qml
    app/StatesPage.qml
    app/TextAction.qml
    app/TodayList.qml
    app/TryItStep.qml
)

# History (fn-22, ADR 0025): the list with search and the filters, the
# detail with the take and the facts, and the three small dialogs.
set(dettivo_history_files
    app/history/AudioStrip.qml
    app/history/DeleteConfirm.qml
    app/history/DetailBlock.qml
    app/history/ExportSheet.qml
    app/history/FactsGrid.qml
    app/history/FilterChips.qml
    app/history/HistoryDetail.qml
    app/history/HistoryList.qml
    app/history/HistoryRoute.qml
    app/history/HistoryRow.qml
    app/history/RerunDialog.qml
    components/SearchField.qml
)

# Meetings (fn-34, ADR 0038): the list by week beside the new-meeting
# rail, the live meeting, the detail with its tabs and rail, and the
# import, disclosure, rename, export and delete dialogs.
set(dettivo_meetings_files
    app/meetings/AnalysisBlock.qml
    app/meetings/AnalysisCard.qml
    app/meetings/AnalysisRail.qml
    app/meetings/AnalysisView.qml
    app/meetings/DetailFollowers.qml
    app/meetings/DetailHeader.qml
    app/meetings/DetailTabs.qml
    app/meetings/DisclosureDialog.qml
    app/meetings/ImportDialog.qml
    app/meetings/ImportField.qml
    app/meetings/ImportFooter.qml
    app/meetings/LevelBars.qml
    app/meetings/LiveHeader.qml
    app/meetings/LiveTranscript.qml
    app/meetings/MeetingDeleteConfirm.qml
    app/meetings/MeetingDetail.qml
    app/meetings/MeetingExportSheet.qml
    app/meetings/MeetingLive.qml
    app/meetings/MeetingRow.qml
    app/meetings/MeetingsFooter.qml
    app/meetings/MeetingsKeys.qml
    app/meetings/MeetingsList.qml
    app/meetings/MeetingsListPage.qml
    app/meetings/MeetingsRoute.qml
    app/meetings/NewMeetingRail.qml
    app/meetings/NotesEditor.qml
    app/meetings/ProcessingStrip.qml
    app/meetings/RailFactRow.qml
    app/meetings/SourceMeter.qml
    app/meetings/SourceRow.qml
    app/meetings/SpeakerRenamePopover.qml
    app/meetings/SpeakerSwatches.qml
    app/meetings/SpeakerTranscript.qml
    app/meetings/StackLayoutLite.qml
    app/meetings/StateChip.qml
    app/meetings/StopConfirm.qml
    app/meetings/TalkTimeBar.qml
    app/meetings/TranscriptRow.qml
)

# Settings (fn-26, ADR 0033): the section column, the pattern (page,
# group, row, the three controls, the TOML block, the fact panel) and the
# eight sections over config.toml.
set(dettivo_settings_files
    app/settings/AgentHostRow.qml
    app/settings/AgentsKeys.qml
    app/settings/AgentsSection.qml
    app/settings/DiagnosticsSection.qml
    app/settings/FactPanel.qml
    app/settings/GeneralSection.qml
    app/settings/HotkeyRow.qml
    app/settings/HotkeysSection.qml
    app/settings/InsertionSection.qml
    app/settings/MeetingsSection.qml
    app/settings/MeetingsAnalysisKeys.qml
    app/settings/MeetingsSpeakerKeys.qml
    app/settings/ModelTableRow.qml
    app/settings/ModelsKeys.qml
    app/settings/ModelsSection.qml
    app/settings/PolishSection.qml
    app/settings/SettingChoice.qml
    app/settings/SettingField.qml
    app/settings/SettingReset.qml
    app/settings/ModelSelections.qml
    app/settings/VocabularySection.qml
    app/settings/TransformOptions.qml
    app/settings/SettingRow.qml
    app/settings/SettingSwitch.qml
    app/settings/SettingsGroup.qml
    app/settings/SettingsNav.qml
    app/settings/SettingsPage.qml
    app/settings/SettingsRoute.qml
    app/settings/SettingsWrites.qml
)

file(GLOB dettivo_icons RELATIVE "${CMAKE_CURRENT_SOURCE_DIR}" CONFIGURE_DEPENDS icons/*.svg)

qt_add_qml_module(Dettivo
    URI Dettivo
    VERSION 1.0
    PLUGIN_TARGET Dettivo
    SOURCES
        theme/theme_backend.h
        theme/theme_backend.cpp
        osd/osd_bars.h
        osd/osd_bars.cpp
    QML_FILES
        ${dettivo_singletons}
        ${dettivo_components}
        ${dettivo_app_files}
        ${dettivo_history_files}
        ${dettivo_meetings_files}
        ${dettivo_settings_files}
    RESOURCES
        ${dettivo_icons}
)

target_include_directories(Dettivo PUBLIC
    "${CMAKE_CURRENT_SOURCE_DIR}/theme"
    "${CMAKE_CURRENT_SOURCE_DIR}/osd")

target_link_libraries(Dettivo
    PUBLIC
        Qt6::Core
        Qt6::Gui
        Qt6::Qml
        Qt6::Quick
        Qt6::DBus
    PRIVATE
        tomlplusplus::tomlplusplus)

set(dettivo_qml_dest "${CMAKE_INSTALL_LIBDIR}/dettivo/qml/Dettivo")

install(TARGETS Dettivo
    LIBRARY DESTINATION "${dettivo_qml_dest}"
    RUNTIME DESTINATION "${dettivo_qml_dest}")

# qmldir and qmltypes are generated into the build tree by qt_add_qml_module;
# the QML sources are installed beside them so tooling can read the module.
install(FILES
    "${QT_QML_OUTPUT_DIRECTORY}/Dettivo/qmldir"
    "${QT_QML_OUTPUT_DIRECTORY}/Dettivo/Dettivo.qmltypes"
    Theme.qml
    SettingsUi.qml
    Motion.qml
    Html.qml
    DESTINATION "${dettivo_qml_dest}")
install(FILES style/StyleHelpers.qml DESTINATION "${dettivo_qml_dest}/style")
install(FILES ${dettivo_components} DESTINATION "${dettivo_qml_dest}/components")
install(FILES ${dettivo_app_files} DESTINATION "${dettivo_qml_dest}/app")
install(FILES ${dettivo_history_files} DESTINATION "${dettivo_qml_dest}/app/history")
install(FILES ${dettivo_meetings_files} DESTINATION "${dettivo_qml_dest}/app/meetings")
install(FILES ${dettivo_settings_files} DESTINATION "${dettivo_qml_dest}/app/settings")
install(FILES ${dettivo_icons} DESTINATION "${dettivo_qml_dest}/icons")

# The six-bar mark is the desktop icon too.
install(FILES icons/dettivo.svg
    DESTINATION "${CMAKE_INSTALL_DATADIR}/icons/hicolor/scalable/apps"
    RENAME dettivo.svg)

add_subdirectory(tests)
