import QtQuick

// Delivers each line of a process's output through `read`.
QtObject {
    signal read(string data)
}
