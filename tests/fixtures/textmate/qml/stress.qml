pragma Singleton
import QtQuick 2.15
import QtQuick.Controls 2.15 as Controls
import QtQuick.Layouts 1.15
import QtQml 2.15
import "widgets" as Local

/*
   Dashboard fixture for a multilingual observatory.
   TODO: preserve the midnight palette; FIXME: verify 🛰 telemetry.
*/
QtObject {
    id: observatory

    enum Mode { Compact, Comfortable, Presentation }
    property string stationName: "North Ridge"
    readonly property string title: `${stationName} — Δελφοί 🔭`
    default property alias cards: observatory.instruments
    property bool online: true
    property int refreshSeconds: 30
    property real scaleFactor: 1.125
    property double threshold: 9.75e2
    property string operatorName: 'Noémie'
    property url endpoint: "https://example.test/telemetry"
    property date lastUpdated: new Date(2026, 6, 12, 20, 15)
    property point origin: Qt.point(12, 18)
    property size preferredSize: Qt.size(960, 640)
    property rect safeArea: Qt.rect(8, 8, 944, 624)
    property variant legacyPayload: ({ source: "archive", revision: 0x2a })
    property var readings: [18.5, 20.25, NaN, Infinity]
    property list<QtObject> instruments: [
        QtObject { property string code: "WX"; property bool active: true },
        QtObject { property string code: "SKY"; property bool active: false }
    ]
    property QtObject metrics: QtObject {
        property int accepted: 0
        property int rejected: 0
    }

    signal sampleAccepted(string instrument, real value, date observedAt)
    signal alertRaised(int severity, string summary)

    //: Accessible description for the station header.
    //= observatory.title
    //~ qsTr("Weather station")
    function normalize(value, minimum, maximum) {
        const span = maximum - minimum;
        if (span <= 0 || !Number.isFinite(value)) {
            return 0;
        }
        return Math.min(1, Math.max(0, (value - minimum) / span));
    }

    function acceptSample(instrument, value) {
        const cleaned = Number(value ?? 0);
        const accepted = cleaned >= 0 && cleaned < threshold;
        if (accepted) {
            metrics.accepted += 1;
            sampleAccepted(instrument, cleaned, new Date());
        } else {
            metrics.rejected++;
            alertRaised(2, `Rejected ${instrument}: ${cleaned}`);
        }
        lastUpdated = new Date();
        return accepted;
    }

    onOnlineChanged: {
        console.info(online ? "telemetry resumed" : "telemetry paused");
        if (!online) {
            alertRaised(1, 'Connection lost');
        }
    }

    property Component dashboard: Component {
        Controls.ApplicationWindow {
            id: window
            visible: true
            width: observatory.preferredSize.width
            height: observatory.preferredSize.height
            title: observatory.title
            color: palette.window

            palette {
                window: "#111827"
                windowText: "#f8fafc"
                button: "#334155"
                highlight: online ? "#22c55e" : "#ef4444"
            }

            background: Rectangle {
                gradient: Gradient {
                    GradientStop { position: 0.0; color: "#172554" }
                    GradientStop { position: 0.55; color: "#1e293b" }
                    GradientStop { position: 1.0; color: "#020617" }
                }
            }

            header: Controls.ToolBar {
                RowLayout {
                    anchors {
                        fill: parent
                        leftMargin: 16
                        rightMargin: 16
                    }
                    spacing: 12

                    Controls.Label {
                        Layout.fillWidth: true
                        text: observatory.title
                        font {
                            bold: true
                            pixelSize: 22
                            letterSpacing: 0.4
                        }
                    }

                    Controls.Switch {
                        id: connectionSwitch
                        text: checked ? "Online" : "Offline"
                        checked: observatory.online
                        onToggled: observatory.online = checked
                    }
                }
            }

            ColumnLayout {
                id: cardColumn
                anchors.fill: parent
                anchors.margins: 24
                spacing: 18

                Controls.Frame {
                    Layout.fillWidth: true
                    padding: 16

                    RowLayout {
                        width: parent.width
                        Controls.Label {
                            text: `Accepted: ${metrics.accepted}`
                            color: "#86efac"
                        }
                        Item { Layout.fillWidth: true }
                        Controls.Label {
                            text: `Rejected: ${metrics.rejected}`
                            color: metrics.rejected > 0 ? "#fca5a5" : "#cbd5e1"
                        }
                    }
                }

                Repeater {
                    model: observatory.instruments
                    delegate: Controls.Frame {
                        required property QtObject modelData
                        Layout.fillWidth: true

                        RowLayout {
                            anchors.fill: parent
                            Controls.Label {
                                Layout.fillWidth: true
                                text: `${modelData.code} instrument`
                            }
                            Controls.ProgressBar {
                                from: 0
                                to: 1
                                value: normalize(Math.random() * 100, 0, 100)
                            }
                            Controls.Button {
                                text: "Sample"
                                enabled: modelData.active && observatory.online
                                onClicked: {
                                    const reading = Math.round(Math.random() * 1000) / 10;
                                    acceptSample(modelData.code, reading);
                                }
                            }
                        }
                    }
                }

                Local.StatusCard {
                    Layout.fillWidth: true
                    heading: "Unicode channel"
                    detail: "naïve façade • 東京 • 🪐"
                    severity: metrics.rejected === 0 ? 0 : 2
                }

                Item { Layout.fillHeight: true }
            }

            footer: Controls.Label {
                horizontalAlignment: Text.AlignHCenter
                padding: 10
                text: `Updated ${lastUpdated.toLocaleString()} by ${operatorName}`
            }

            Shortcut {
                sequence: "Ctrl+R"
                onActivated: acceptSample("manual", readings[0] * scaleFactor)
            }

            Component.onCompleted: {
                for (const instrument of instruments) {
                    console.debug(`loaded ${instrument.code}`);
                }
            }
        }
    }
}
