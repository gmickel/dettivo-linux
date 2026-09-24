import QtQuick
import QtTest
import Dettivo

TestCase {
    id: root
    name: "CleanupImport"
    width: 900
    height: 900
    visible: true
    when: windowShown
    Component {
        id: dialogC
        ImportDialog {}
    }
    Component {
        id: actionsC
        FakeMeetingsActions {
            property var submitted: []
            function importFile(path, provider, model, language, diarize, analyse) {
                submitted = [path, provider, model, language, diarize, analyse];
            }
        }
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
    function test_meeting_import_has_only_submittable_options() {
        const actions = createTemporaryObject(actionsC, root);
        const dialog = createTemporaryObject(dialogC, root, {
            actions: actions
        });
        dialog.openSample();
        tryCompare(dialog, "opened", true);
        compare(named(dialog.contentItem, "Target kind"), null);
        const button = named(dialog.contentItem, "Import");
        verify(button.enabled);
        button.forceActiveFocus(Qt.TabFocusReason);
        keyClick(Qt.Key_Space);
        compare(actions.submitted[0], "interview-raw.m4a");
        compare(actions.submitted[3], "auto");
        compare(actions.submitted[4], true);
        compare(actions.submitted[5], true);
    }
}
