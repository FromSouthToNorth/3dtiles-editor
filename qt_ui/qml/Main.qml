import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import EditorUI

ApplicationWindow {
    id: win
    width: 1480
    height: 920
    visible: true
    color: "#11161f"
    title: engineBridge.tilesetLoaded
        ? ("3dtiles-editor - " + engineBridge.currentPath)
        : "3dtiles-editor (Qt 6 + Rust + wgpu)"

    menuBar: MenuBar {
        Menu {
            title: qsTr("文件(&F)")
            Action { text: qsTr("打开 tileset.json..."); shortcut: StandardKey.Open; onTriggered: engineBridge.openDialog() }
            Action { text: qsTr("保存"); shortcut: StandardKey.Save; enabled: engineBridge.tilesetLoaded; onTriggered: engineBridge.saveCurrent() }
            Action { text: qsTr("另存为..."); enabled: engineBridge.tilesetLoaded; onTriggered: engineBridge.saveAsDialog() }
            MenuSeparator {}
            Action { text: qsTr("退出"); onTriggered: Qt.quit() }
        }
        Menu {
            title: qsTr("编辑(&E)")
            Action { text: qsTr("撤销"); shortcut: StandardKey.Undo; onTriggered: engineBridge.undo() }
            Action { text: qsTr("重做"); shortcut: StandardKey.Redo; onTriggered: engineBridge.redo() }
        }
        Menu {
            title: qsTr("视图(&V)")
            Action { text: qsTr("定位根节点"); enabled: engineBridge.tilesetLoaded; onTriggered: engineBridge.focusTile(0) }
        }
    }

    header: ToolBar {
        background: Rectangle { color: "#171e2b" }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 10
            anchors.rightMargin: 10
            spacing: 10

            Button { text: qsTr("打开"); onClicked: engineBridge.openDialog() }
            Button { text: qsTr("保存"); enabled: engineBridge.tilesetLoaded; onClicked: engineBridge.saveCurrent() }
            Button { text: qsTr("聚焦总览"); enabled: engineBridge.tilesetLoaded; onClicked: engineBridge.focusTile(0) }

            CheckBox {
                id: bboxToggle
                text: qsTr("显示所有包围盒")
                onToggled: engineBridge.setShowAllBBoxes(checked)
            }

            Label { text: qsTr("SSE 阈值:"); color: "#cbd5e1" }
            Slider {
                id: sseSlider
                from: 2.0
                to: 32.0
                value: 8.0
                stepSize: 1.0
                implicitWidth: 140
                onMoved: engineBridge.setMaxSse(value)
            }
            Label { text: sseSlider.value.toFixed(0) + " px"; color: "#38bdf8" }

            Item { Layout.fillWidth: true }
            Label {
                text: engineBridge.statsText
                color: "#94a3b8"
                font.pixelSize: 11
                elide: Text.ElideRight
                Layout.maximumWidth: 520
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true

            SceneTreePanel {
                SplitView.preferredWidth: 280
                SplitView.minimumWidth: 200
            }

            // 中央原生 WebGPU 3D 视口，严格保持无重叠 QML 控件遮挡
            Item {
                SplitView.fillWidth: true
                SplitView.minimumWidth: 400
                ViewportItem {
                    anchors.fill: parent
                    engine: engineBridge
                }
            }

            InspectorPanel {
                SplitView.preferredWidth: 330
                SplitView.minimumWidth: 260
            }
        }

        LogBar {
            Layout.fillWidth: true
            Layout.preferredHeight: 30
        }
    }
}
