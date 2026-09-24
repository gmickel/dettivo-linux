import QtQuick
import QtTest
import Dettivo

TestCase {
    id: root
    name: "MeetingExportNotes"
    width: 1280
    height: 820
    visible: true
    when: windowShown

    Component {
        id: paneC
        MeetingDetail {
            width: 1080
            height: 800
            meetingId: "m1"
        }
    }
    Component {
        id: detailC
        FakeMeetingDetail {}
    }
    Component {
        id: actionsC
        FakeHistoryActions {}
    }

    function named(item, name) {
        for (const child of item.children) {
            if (child.Accessible && child.Accessible.name === name)
                return child;
            const found = named(child, name);
            if (found)
                return found;
        }
        return null;
    }

    function test_export_sends_current_editor_notes_data() {
        return [
            {
                tag: "cleared",
                draft: ""
            },
            {
                tag: "unsaved",
                draft: "Unsaved notes"
            }
        ];
    }

    function test_export_sends_current_editor_notes(data) {
        const detail = createTemporaryObject(detailC, root);
        const actions = createTemporaryObject(actionsC, root);
        const pane = createTemporaryObject(paneC, root, {
            detail: detail,
            actions: actions
        });
        pane.pickTab(1);
        const editor = named(pane, "Notes editor");
        verify(editor);
        editor.forceActiveFocus();
        editor.selectAll();
        keyClick(Qt.Key_Backspace);
        editor.insert(0, data.draft);
        compare(detail.notes, data.draft);
        pane.exportSheet();
        const overlay = root.Window.window.contentItem;
        const raw = named(overlay, "The engine's words instead of the polished segments");
        verify(raw);
        verify(raw.width <= raw.parent.width, "Export option stays inside the content column");
        verify(raw.contentItem.paintedWidth <= raw.contentItem.width, "Export option wraps within its width");
        verify(raw.contentItem.paintedHeight <= raw.contentItem.height, "Wrapped export option remains fully visible");
        const write = named(overlay, "Write file");
        verify(write);
        mouseClick(write);
        compare(actions.exports.length, 1);
        compare(actions.exports[0].notesOverride, data.draft);
        mouseClick(named(overlay, "Close"));
    }
}
