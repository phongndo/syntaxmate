import QtQuick 2.15
import QtQuick.Controls 2.15 as Controls

// Basic greeting: café, λ, and 🚀
Rectangle {
    id: root
    required property string visitor
    readonly property real scaleFactor: 1.25
    property color accent: "#4f7cff"
    signal welcomed(string name, int count)
    width: 320; height: 180
    color: accent

    Controls.Label {
        anchors.centerIn: parent
        text: `Hello, ${root.visitor} — 世界 🌍`
    }

    function greet(name) { welcomed(name, 1); return name.toUpperCase(); }
    Component.onCompleted: console.log(greet(visitor))
}
