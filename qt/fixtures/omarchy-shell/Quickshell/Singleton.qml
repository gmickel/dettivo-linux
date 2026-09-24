import QtQuick

// The root type of a Quickshell singleton file; children are kept so the
// processes and timers inside it instantiate.
QtObject {
    default property list<QtObject> data
}
