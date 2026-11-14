import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    color: "#151b26"
    border.color: "#243044"
    border.width: 1

    function commitTrs() {
        engineBridge.applySelectedTrs(
            parseFloat(txField.text) || 0,
            parseFloat(tyField.text) || 0,
            parseFloat(tzField.text) || 0,
            parseFloat(rxField.text) || 0,
            parseFloat(ryField.text) || 0,
            parseFloat(rzField.text) || 0,
            parseFloat(sxField.text) || 1,
            parseFloat(syField.text) || 1,
            parseFloat(szField.text) || 1
        )
    }

    ScrollView {
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: parent.width - 20
            x: 10
            y: 10
            spacing: 10

            Label {
                text: qsTr("节点检查器")
                color: "#e2e8f0"
                font.bold: true
                font.pixelSize: 15
            }

            Label {
                visible: engineBridge.selectedTileId === 0
                text: qsTr("未选中任何节点\n提示: 左键旋转 / 右键平移 / 滚轮缩放 / 点击拾取")
                color: "#64748b"
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }

            ColumnLayout {
                visible: engineBridge.selectedTileId !== 0
                Layout.fillWidth: true
                spacing: 8

                Label {
                    text: "ID #" + engineBridge.selectedTileId + "  " + engineBridge.selectedLabel
                    color: "#38bdf8"
                    font.bold: true
                    wrapMode: Text.WrapAnywhere
                    Layout.fillWidth: true
                }

                Label {
                    text: engineBridge.selectedMetadata
                    color: "#cbd5e1"
                    font.pixelSize: 12
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }

                RowLayout {
                    CheckBox {
                        text: qsTr("节点可见")
                        checked: engineBridge.selectedVisible
                        onToggled: engineBridge.setSelectedVisible(checked)
                    }
                    Item { Layout.fillWidth: true }
                    Button {
                        text: qsTr("聚焦此节点")
                        onClicked: engineBridge.focusTile(engineBridge.selectedTileId)
                    }
                }

                GroupBox {
                    title: qsTr("局部变换 (TRS)")
                    Layout.fillWidth: true
                    label: Label { text: parent.title; color: "#e2e8f0"; font.bold: true }
                    background: Rectangle { color: "#1b2332"; border.color: "#2b3950"; radius: 4 }

                    GridLayout {
                        anchors.fill: parent
                        columns: 4
                        rowSpacing: 6
                        columnSpacing: 4

                        Label { text: "T"; color: "#94a3b8" }
                        TextField { id: txField; text: engineBridge.posX.toFixed(3); Layout.fillWidth: true; onEditingFinished: root.commitTrs() }
                        TextField { id: tyField; text: engineBridge.posY.toFixed(3); Layout.fillWidth: true; onEditingFinished: root.commitTrs() }
                        TextField { id: tzField; text: engineBridge.posZ.toFixed(3); Layout.fillWidth: true; onEditingFinished: root.commitTrs() }

                        Label { text: "R°"; color: "#94a3b8" }
                        TextField { id: rxField; text: engineBridge.rotX.toFixed(2); Layout.fillWidth: true; onEditingFinished: root.commitTrs() }
                        TextField { id: ryField; text: engineBridge.rotY.toFixed(2); Layout.fillWidth: true; onEditingFinished: root.commitTrs() }
                        TextField { id: rzField; text: engineBridge.rotZ.toFixed(2); Layout.fillWidth: true; onEditingFinished: root.commitTrs() }

                        Label { text: "S"; color: "#94a3b8" }
                        TextField { id: sxField; text: engineBridge.scaleX.toFixed(3); Layout.fillWidth: true; onEditingFinished: root.commitTrs() }
                        TextField { id: syField; text: engineBridge.scaleY.toFixed(3); Layout.fillWidth: true; onEditingFinished: root.commitTrs() }
                        TextField { id: szField; text: engineBridge.scaleZ.toFixed(3); Layout.fillWidth: true; onEditingFinished: root.commitTrs() }
                    }
                }

                Button {
                    text: qsTr("应用变换")
                    Layout.fillWidth: true
                    onClicked: root.commitTrs()
                }
            }
        }
    }
}
