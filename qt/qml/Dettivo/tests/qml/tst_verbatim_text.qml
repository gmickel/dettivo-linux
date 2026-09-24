import QtQuick
import QtTest
import Dettivo

TestCase {
    id: root
    name: "VerbatimText"
    width: 1200
    height: 800
    visible: true
    when: windowShown

    Component {
        id: liveModelC
        FakeMeetingLive {}
    }
    Component {
        id: detailModelC
        FakeMeetingDetail {}
    }
    Component {
        id: liveHeaderC
        LiveHeader {
            width: 900
        }
    }
    Component {
        id: detailHeaderC
        DetailHeader {
            width: 900
        }
    }
    Component {
        id: swatchesC
        SpeakerSwatches {
            width: 900
        }
    }
    Component {
        id: talkC
        TalkTimeBar {
            width: 900
        }
    }
    Component {
        id: renameC
        SpeakerRenamePopover {}
    }
    Component {
        id: actionsC
        FakeMeetingsActions {}
    }

    Component {
        id: transcriptC
        TranscriptRow {
            width: 900
        }
    }
    Component {
        id: detailC
        DetailBlock {
            width: 900
        }
    }
    Component {
        id: listC
        ListRow {
            width: 900
        }
    }
    Component {
        id: meetingC
        MeetingRow {
            width: 900
        }
    }
    Component {
        id: analysisC
        AnalysisBlock {
            width: 900
        }
    }

    function textItem(item, text) {
        for (const child of item.children) {
            if (child.text === text && child.textFormat !== undefined)
                return child;
            const nested = textItem(child, text);
            if (nested)
                return nested;
        }
        return null;
    }

    function test_literal_content_data() {
        return [
            {
                tag: "live-title",
                component: liveHeaderC,
                field: "live"
            },
            {
                tag: "detail-title",
                component: detailHeaderC,
                field: "detail"
            },
            {
                tag: "speaker-swatches",
                component: swatchesC,
                field: "speakers"
            },
            {
                tag: "speaker-legend",
                component: talkC,
                field: "speakers"
            },
            {
                tag: "speaker-suggestion",
                component: renameC,
                field: "actions"
            },
            {
                tag: "transcript",
                component: transcriptC,
                field: "text"
            },
            {
                tag: "speaker",
                component: transcriptC,
                field: "label"
            },
            {
                tag: "history",
                component: detailC,
                field: "body"
            },
            {
                tag: "today",
                component: listC,
                field: "text"
            },
            {
                tag: "meeting-title",
                component: meetingC,
                field: "title"
            },
            {
                tag: "meeting-summary",
                component: meetingC,
                field: "summary"
            },
            {
                tag: "analysis-paragraph",
                component: analysisC,
                field: "paragraph"
            },
            {
                tag: "analysis-list",
                component: analysisC,
                field: "items"
            }
        ];
    }

    function test_literal_content(data) {
        const literal = "<b>code & names</b>\nif (a < b) return;";
        const properties = {};
        properties[data.field] = data.field === "items" ? [literal] : literal;
        if (data.field === "live" || data.field === "detail")
            properties[data.field] = createTemporaryObject(data.field === "live" ? liveModelC : detailModelC, root, {
                title: literal
            });
        if (data.field === "speakers")
            properties[data.field] = [
                {
                    name: literal,
                    colorIndex: 0,
                    speakerId: "one",
                    talkMs: 1000,
                    talk: "1 s",
                    share: 1,
                    shareText: "100 %"
                }
            ];
        if (data.field === "actions")
            properties[data.field] = createTemporaryObject(actionsC, root, {
                suggestions: [
                    {
                        name: literal,
                        uses: 1
                    }
                ]
            });
        const surface = createTemporaryObject(data.component, root, properties);
        const text = textItem(surface.contentItem || surface, data.field === "items" ? "– " + literal : literal);
        verify(text);
        compare(text.textFormat, Text.PlainText, "verbatim content never uses automatic markup detection");
    }
}
