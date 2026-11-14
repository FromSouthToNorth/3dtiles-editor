import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    color: "#0f141c"
    border.color: "#1e293b"
    border.width: 1

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 10
        anchors.rightMargin: 10
        spacing: 12

        Label {
            text: engineBridge.errorMessage.length > 0
                ? ("[错误] " + engineBridge.errorMessage)
                : engineBridge.lastLog
            color: engineBridge.errorMessage.length > 0 ? "#f87171" : "#94a3b8"
            elide: Text.ElideRight
            Layout.fillWidth: true
            font.pixelSize: 12
        }

        ProgressBar {
            visible: engineBridge.isLoading
            value: engineBridge.loadProgress
            implicitWidth: 160
        }
    }
}
