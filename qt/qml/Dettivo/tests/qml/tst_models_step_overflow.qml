pragma ComponentBehavior: Bound
import Dettivo
import QtQuick
import QtTest

// English adds a fifth speech row (the Parakeet v2 recommendation): at the
// first-run window size the footer stays below the Enhanced rows and the
// page scrolls to reach it instead of drawing over them.
TestCase {
    id: root
    name: "ModelsStepOverflow"
    width: 1280
    height: 820
    visible: true
    when: windowShown

    function findByName(item, name) {
        for (const child of item.children) {
            if (child.Accessible && child.Accessible.name === name)
                return child;
            const nested = findByName(child, name);
            if (nested)
                return nested;
        }
        return null;
    }

    function englishRoute() {
        const models = createTemporaryObject(modelsC, root);
        for (const name of ["Whisper small.en", "Whisper large-v3-turbo", "Parakeet TDT 0.6B v2 (English)", "Parakeet TDT 0.6B v3", "Parakeet Ultra 0.6B"])
            models.append({
                "provider": "parakeet",
                "modelId": name,
                "name": name,
                "detail": "detail",
                "size": "941 MB",
                "state": "get",
                "recommended": false,
                "selected": false,
                "ready": false
            });
        const firstRun = createTemporaryObject(firstRunC, root, {
            "models": models
        });
        const page = createTemporaryObject(routeC, root, {
            "firstRun": firstRun,
            "width": 1280,
            "height": 820
        });
        waitForRendering(page);
        return page;
    }

    function test_five_speech_rows_scroll_instead_of_overlapping() {
        const page = englishRoute();
        const enhanced = findByName(page, "Enhanced models");
        const footer = findByName(page, "First run actions");
        const enhancedBottom = enhanced.mapToItem(page.contentItem, 0, enhanced.height).y;
        const footerTop = footer.mapToItem(page.contentItem, 0, 0).y;
        verify(footerTop >= enhancedBottom + Theme.space5, `footer at ${footerTop} overlaps the Enhanced rows ending at ${enhancedBottom}`);
        verify(page.contentHeight > page.height, "the page does not scroll");
        verify(footer.mapToItem(page.contentItem, 0, footer.height).y <= page.contentHeight);
    }

    // Tab to Back scrolls the whole footer into view, its two-line note
    // included, not just the button.
    function test_focusing_a_footer_button_reveals_the_whole_footer() {
        const page = englishRoute();
        const footer = findByName(page, "First run actions");
        const back = findByName(footer, "Back");
        verify(back);
        back.forceActiveFocus();
        tryVerify(() => back.mapToItem(page, 0, back.height).y <= page.height);
        const footerBottom = footer.mapToItem(page, 0, footer.height).y;
        verify(footerBottom <= page.height, `footer ends at ${footerBottom}, below the ${page.height} px window`);
    }

    Component {
        id: routeC

        OnboardingRoute {}
    }

    Component {
        id: modelsC

        ListModel {
            property bool ready: false
            property string modelsDir: "~/.local/share/dettivo/models"
            property string tierLine: "Vulkan will run them."
            property bool downloading: false
            property real downloadFraction: 0
            property string downloadFile: ""
            property string downloadLine: ""
            property string enhanced: "none"
            property string localTitle: "Qwen3 4B Instruct, local"
            property string localLine: "Rewrites dictation in Enhanced mode and writes meeting analysis. Runs in its own process, unloads when idle."
            property string localSize: "2.5 GB"
            property string localState: "get"
            property string ollamaLine: "Not running on localhost:11434. Polish still works without any model."
            property bool ollamaAvailable: false
        }
    }

    Component {
        id: firstRunC

        QtObject {
            property string step: "models"
            property int stepIndex: 1
            property var models: null
        }
    }
}
