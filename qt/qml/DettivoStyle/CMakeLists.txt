# The `DettivoStyle` Qt Quick Controls style (ADR 0010): one control file per
# Quick Controls type, every state drawn from the shell's fill and border
# alphas. Qt resolves a style as a QML module by name, so the hosts call
# QQuickStyle::setStyle("DettivoStyle") and Qt finds this module on the
# import path in the build tree and under /usr/lib/dettivo/qml once
# installed.
qt_add_library(DettivoStyle SHARED)

set(dettivo_style_files
    Button.qml
    SearchClearButton.qml
    SidebarButton.qml
    ModelChoice.qml
    SegmentButton.qml
    CheckBox.qml
    ComboBox.qml
    ItemDelegate.qml
    Menu.qml
    MenuItem.qml
    Popup.qml
    ProgressBar.qml
    RadioButton.qml
    ScrollBar.qml
    Switch.qml
    TabBar.qml
    TabButton.qml
    TextArea.qml
    TextField.qml
    ToolTip.qml
)

qt_add_qml_module(DettivoStyle
    URI DettivoStyle
    VERSION 1.0
    PLUGIN_TARGET DettivoStyle
    QML_FILES ${dettivo_style_files}
)

target_link_libraries(DettivoStyle
    PUBLIC
        Qt6::Core
        Qt6::Gui
        Qt6::Qml
        Qt6::Quick
        Qt6::QuickControls2
        Dettivo)

set(dettivo_style_dest "${CMAKE_INSTALL_LIBDIR}/dettivo/qml/DettivoStyle")

# The style links libDettivo.so from the sibling module directory.
set_target_properties(DettivoStyle PROPERTIES
    INSTALL_RPATH "${DETTIVO_QML_INSTALL_DIR}/Dettivo")

install(TARGETS DettivoStyle
    LIBRARY DESTINATION "${dettivo_style_dest}"
    RUNTIME DESTINATION "${dettivo_style_dest}")
install(FILES
    "${QT_QML_OUTPUT_DIRECTORY}/DettivoStyle/qmldir"
    "${QT_QML_OUTPUT_DIRECTORY}/DettivoStyle/DettivoStyle.qmltypes"
    ${dettivo_style_files}
    DESTINATION "${dettivo_style_dest}")
