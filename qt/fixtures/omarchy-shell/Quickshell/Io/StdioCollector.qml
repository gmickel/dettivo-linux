import QtQuick

// Collects a process's whole output into `text`.
QtObject {
    property string text: ""
    property bool waitForEnd: true
    signal streamFinished
}
