import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import EditorUI

ApplicationWindow {
    id: win
    width: 1520
    height: 940
    visible: true
    color: Theme.bgApp
    title: engineBridge.tilesetLoaded
        ? ("3D Tiles Editor — " + engineBridge.currentPath)
        : "3D Tiles Editor (Qt 6 + Rust + WebGPU)"

    menuBar: MenuBar {
        background: Rectangle {
            color: Theme.bgHeader
            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: 1
                color: Theme.borderSubtle
            }
        }

        delegate: MenuBarItem {
            id: menuBarItem
            contentItem: Text {
                text: menuBarItem.text
                font.pixelSize: 13
                color: menuBarItem.highlighted ? Theme.accent : Theme.textSecondary
                horizontalAlignment: Text.AlignLeft
                verticalAlignment: Text.AlignVCenter
                elide: Text.ElideRight
            }
            background: Rectangle {
                color: menuBarItem.highlighted ? Theme.bgCardHover : "transparent"
                radius: Theme.radiusSmall
            }
        }

        Menu {
            title: qsTr("文件 (&F)")
            Action { text: qsTr("打开 tileset.json..."); shortcut: StandardKey.Open; onTriggered: engineBridge.openDialog() }
            Action { text: qsTr("保存修改"); shortcut: StandardKey.Save; enabled: engineBridge.tilesetLoaded; onTriggered: engineBridge.saveCurrent() }
            Action { text: qsTr("另存为..."); enabled: engineBridge.tilesetLoaded; onTriggered: engineBridge.saveAsDialog() }
            MenuSeparator {}
            Action { text: qsTr("退出"); onTriggered: Qt.quit() }
        }
        Menu {
            title: qsTr("编辑 (&E)")
            Action { text: qsTr("撤销 (Undo)"); shortcut: StandardKey.Undo; onTriggered: engineBridge.undo() }
            Action { text: qsTr("重做 (Redo)"); shortcut: StandardKey.Redo; onTriggered: engineBridge.redo() }
        }
        Menu {
            title: qsTr("视图 (&V)")
            Action { text: qsTr("定位根节点全景"); enabled: engineBridge.tilesetLoaded; onTriggered: engineBridge.focusTile(0) }
            Action { text: qsTr("显示包围盒线框"); checkable: true; checked: false; onToggled: engineBridge.setShowAllBBoxes(checked) }
        }
    }

    header: ToolBar {
        height: 46
        background: Rectangle {
            color: Theme.bgHeader
            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: 1
                color: Theme.borderSubtle
            }
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            spacing: 8

            // --- 文件操作组 ---
            RowLayout {
                spacing: 4
                Button {
                    id: btnOpen
                    text: qsTr("打开文件")
                    font.pixelSize: 12
                    font.bold: true
                    contentItem: Text {
                        text: btnOpen.text
                        color: btnOpen.hovered ? Theme.textPrimary : Theme.textSecondary
                        font: btnOpen.font
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    background: Rectangle {
                        implicitWidth: 72
                        implicitHeight: 28
                        color: btnOpen.pressed ? Theme.bgCardHover : (btnOpen.hovered ? Theme.bgInputHover : Theme.bgInput)
                        border.color: btnOpen.hovered ? Theme.borderFocus : Theme.borderMedium
                        border.width: 1
                        radius: Theme.radiusSmall
                    }
                    onClicked: engineBridge.openDialog()
                }

                Button {
                    id: btnSave
                    text: qsTr("保存")
                    enabled: engineBridge.tilesetLoaded
                    font.pixelSize: 12
                    font.bold: true
                    contentItem: Text {
                        text: btnSave.text
                        color: btnSave.enabled ? (btnSave.hovered ? "#ffffff" : Theme.accent) : Theme.textDisabled
                        font: btnSave.font
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    background: Rectangle {
                        implicitWidth: 60
                        implicitHeight: 28
                        color: btnSave.enabled ? (btnSave.hovered ? Theme.accentHover : Theme.accentDim) : Theme.bgInput
                        border.color: btnSave.enabled ? Theme.accent : Theme.borderSubtle
                        border.width: 1
                        radius: Theme.radiusSmall
                    }
                    onClicked: engineBridge.saveCurrent()
                }
            }

            Rectangle { width: 1; height: 18; color: Theme.borderMedium; Layout.leftMargin: 4; Layout.rightMargin: 4 }

            // --- 历史撤销重做组 ---
            RowLayout {
                spacing: 4
                Button {
                    id: btnUndo
                    text: qsTr("撤销")
                    font.pixelSize: 12
                    contentItem: Text {
                        text: btnUndo.text
                        color: btnUndo.hovered ? Theme.textPrimary : Theme.textSecondary
                        font: btnUndo.font
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    background: Rectangle {
                        implicitWidth: 54
                        implicitHeight: 28
                        color: btnUndo.hovered ? Theme.bgInputHover : Theme.bgInput
                        border.color: Theme.borderMedium
                        border.width: 1
                        radius: Theme.radiusSmall
                    }
                    onClicked: engineBridge.undo()
                }

                Button {
                    id: btnRedo
                    text: qsTr("重做")
                    font.pixelSize: 12
                    contentItem: Text {
                        text: btnRedo.text
                        color: btnRedo.hovered ? Theme.textPrimary : Theme.textSecondary
                        font: btnRedo.font
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    background: Rectangle {
                        implicitWidth: 54
                        implicitHeight: 28
                        color: btnRedo.hovered ? Theme.bgInputHover : Theme.bgInput
                        border.color: Theme.borderMedium
                        border.width: 1
                        radius: Theme.radiusSmall
                    }
                    onClicked: engineBridge.redo()
                }
            }

            Rectangle { width: 1; height: 18; color: Theme.borderMedium; Layout.leftMargin: 4; Layout.rightMargin: 4 }

            // --- 视效与漫游组 ---
            RowLayout {
                spacing: 4
                Button {
                    id: btnFit
                    text: qsTr("聚焦全景")
                    enabled: engineBridge.tilesetLoaded
                    font.pixelSize: 12
                    contentItem: Text {
                        text: btnFit.text
                        color: btnFit.enabled ? (btnFit.hovered ? Theme.textPrimary : Theme.textSecondary) : Theme.textDisabled
                        font: btnFit.font
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    background: Rectangle {
                        implicitWidth: 72
                        implicitHeight: 28
                        color: btnFit.hovered ? Theme.bgInputHover : Theme.bgInput
                        border.color: Theme.borderMedium
                        border.width: 1
                        radius: Theme.radiusSmall
                    }
                    onClicked: engineBridge.focusTile(0)
                }

                Button {
                    id: btnBBox
                    text: qsTr("包围盒")
                    checkable: true
                    checked: false
                    font.pixelSize: 12
                    font.bold: checked
                    contentItem: Text {
                        text: (btnBBox.checked ? "● " : "○ ") + btnBBox.text
                        color: btnBBox.checked ? Theme.accent : Theme.textMuted
                        font: btnBBox.font
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    background: Rectangle {
                        implicitWidth: 76
                        implicitHeight: 28
                        color: btnBBox.checked ? Theme.accentDim : Theme.bgInput
                        border.color: btnBBox.checked ? Theme.borderFocus : Theme.borderMedium
                        border.width: 1
                        radius: Theme.radiusSmall
                    }
                    onToggled: engineBridge.setShowAllBBoxes(checked)
                }
            }

            Rectangle { width: 1; height: 18; color: Theme.borderMedium; Layout.leftMargin: 4; Layout.rightMargin: 4 }

            // --- SSE 误差调度胶囊 ---
            RowLayout {
                spacing: 6
                Rectangle {
                    implicitWidth: 200
                    implicitHeight: 28
                    color: Theme.bgInput
                    border.color: Theme.borderMedium
                    border.width: 1
                    radius: Theme.radiusSmall

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 8
                        anchors.rightMargin: 8
                        spacing: 6

                        Text {
                            text: "SSE"
                            color: Theme.textMuted
                            font.pixelSize: 11
                            font.bold: true
                        }

                        Slider {
                            id: sseSlider
                            from: 2.0
                            to: 32.0
                            value: 8.0
                            stepSize: 1.0
                            Layout.fillWidth: true
                            onMoved: engineBridge.setMaxSse(value)

                            background: Rectangle {
                                x: sseSlider.leftPadding
                                y: sseSlider.topPadding + sseSlider.availableHeight / 2 - 2
                                width: sseSlider.availableWidth
                                height: 4
                                radius: 2
                                color: "#1e293b"

                                Rectangle {
                                    width: sseSlider.visualPosition * parent.width
                                    height: parent.height
                                    color: Theme.accent
                                    radius: 2
                                }
                            }

                            handle: Rectangle {
                                x: sseSlider.leftPadding + sseSlider.visualPosition * (sseSlider.availableWidth - width)
                                y: sseSlider.topPadding + sseSlider.availableHeight / 2 - height / 2
                                width: 12
                                height: 12
                                radius: 6
                                color: sseSlider.pressed ? "#ffffff" : Theme.accent
                                border.color: "#0f172a"
                                border.width: 1
                            }
                        }

                        Text {
                            text: sseSlider.value.toFixed(0) + "px"
                            color: Theme.accent
                            font.pixelSize: 11
                            font.bold: true
                        }
                    }
                }
            }

            Item { Layout.fillWidth: true }

            // --- 状态指示芯片 (Status Pill) ---
            Rectangle {
                implicitHeight: 28
                implicitWidth: statusText.implicitWidth + 24
                color: Theme.bgCard
                border.color: Theme.borderMedium
                border.width: 1
                radius: Theme.radiusSmall

                RowLayout {
                    anchors.centerIn: parent
                    spacing: 8

                    Rectangle {
                        width: 7
                        height: 7
                        radius: 3.5
                        color: Theme.success
                    }

                    Text {
                        id: statusText
                        text: engineBridge.statsText.length > 0 ? engineBridge.statsText : "GPU 渲染器就绪"
                        color: Theme.textSecondary
                        font.pixelSize: 11
                        font.family: "Segoe UI, PingFang SC, sans-serif"
                    }
                }
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Horizontal

            handle: Rectangle {
                implicitWidth: 3
                color: SplitHandle.hovered || SplitHandle.pressed ? Theme.accent : Theme.borderSubtle
                Behavior on color { ColorAnimation { duration: 150 } }
            }

            SceneTreePanel {
                SplitView.preferredWidth: 300
                SplitView.minimumWidth: 220
                SplitView.maximumWidth: 500
            }

            // 中央原生 WebGPU 3D 视口
            Item {
                SplitView.fillWidth: true
                SplitView.minimumWidth: 420
                clip: true

                ViewportItem {
                    anchors.fill: parent
                    engine: engineBridge
                }
            }

            InspectorPanel {
                SplitView.preferredWidth: 360
                SplitView.minimumWidth: 280
                SplitView.maximumWidth: 560
            }
        }

        LogBar {
            Layout.fillWidth: true
            Layout.preferredHeight: 34
        }
    }
}
