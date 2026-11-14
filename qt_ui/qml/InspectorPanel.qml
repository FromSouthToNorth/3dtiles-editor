import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import EditorUI

Rectangle {
    id: root
    color: Theme.bgPanel
    border.color: Theme.borderSubtle
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

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // --- 检查器顶栏 ---
        Rectangle {
            Layout.fillWidth: true
            height: 38
            color: Theme.bgPanelHeader

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 12

                Text {
                    text: qsTr("属性检查器")
                    color: Theme.textPrimary
                    font.bold: true
                    font.pixelSize: 13
                }

                Item { Layout.fillWidth: true }

                Rectangle {
                    visible: engineBridge.selectedTileId !== 0
                    implicitWidth: idTag.implicitWidth + 12
                    implicitHeight: 20
                    radius: 10
                    color: Theme.accentDim
                    border.color: Theme.accent
                    border.width: 1

                    Text {
                        id: idTag
                        anchors.centerIn: parent
                        text: "ID #" + engineBridge.selectedTileId
                        color: Theme.accent
                        font.pixelSize: 10
                        font.bold: true
                    }
                }
            }

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: 1
                color: Theme.borderSubtle
            }
        }

        // --- 内容滚动区域 ---
        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: availableWidth

            ScrollBar.vertical: ScrollBar {
                policy: ScrollBar.AsNeeded
                contentItem: Rectangle {
                    implicitWidth: 6
                    radius: 3
                    color: Theme.borderMedium
                }
            }

            ColumnLayout {
                width: parent.width - 24
                x: 12
                y: 12
                spacing: 12

                // === 空状态占位卡 ===
                Rectangle {
                    visible: engineBridge.selectedTileId === 0
                    Layout.fillWidth: true
                    implicitHeight: 220
                    color: Theme.bgCard
                    border.color: Theme.borderMedium
                    border.width: 1
                    radius: Theme.radiusMedium

                    ColumnLayout {
                        anchors.centerIn: parent
                        spacing: 10
                        width: parent.width - 32

                        Text {
                            text: "📐"
                            font.pixelSize: 32
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                        }

                        Text {
                            text: qsTr("未选中任何瓦片")
                            color: Theme.textPrimary
                            font.bold: true
                            font.pixelSize: 14
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                        }

                        Text {
                            text: qsTr("请在 3D 视口中鼠标左键单击模型，或在左侧大纲树中选择任意节点。")
                            color: Theme.textMuted
                            font.pixelSize: 12
                            wrapMode: Text.WordWrap
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                        }

                        Rectangle { width: 1; height: 6 }

                        Rectangle {
                            Layout.fillWidth: true
                            height: 1
                            color: Theme.borderSubtle
                        }

                        Text {
                            text: qsTr("漫游提示：左键旋转 / 右键平移 / 滚轮缩放")
                            color: Theme.textDisabled
                            font.pixelSize: 11
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                        }
                    }
                }

                // === 选中节点主面板 ===
                ColumnLayout {
                    visible: engineBridge.selectedTileId !== 0
                    Layout.fillWidth: true
                    spacing: 12

                    // --- 卡片 1: 节点头部与快捷控制 ---
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: colHeader.implicitHeight + 20
                        color: Theme.bgCard
                        border.color: Theme.borderMedium
                        border.width: 1
                        radius: Theme.radiusMedium

                        ColumnLayout {
                            id: colHeader
                            anchors.fill: parent
                            anchors.margins: 10
                            spacing: 8

                            Text {
                                text: engineBridge.selectedLabel
                                color: Theme.accent
                                font.bold: true
                                font.pixelSize: 13
                                wrapMode: Text.WrapAnywhere
                                Layout.fillWidth: true
                            }

                            RowLayout {
                                spacing: 8

                                // 节点显隐 Toggle
                                Button {
                                    id: btnVis
                                    text: engineBridge.selectedVisible ? qsTr("👁 节点可见") : qsTr("🚫 节点隐藏")
                                    checkable: true
                                    checked: engineBridge.selectedVisible
                                    font.pixelSize: 11
                                    contentItem: Text {
                                        text: btnVis.text
                                        color: btnVis.checked ? Theme.success : Theme.textDisabled
                                        font: btnVis.font
                                        horizontalAlignment: Text.AlignHCenter
                                        verticalAlignment: Text.AlignVCenter
                                    }
                                    background: Rectangle {
                                        implicitWidth: 90
                                        implicitHeight: 26
                                        color: btnVis.checked ? Theme.successBg : Theme.bgInput
                                        border.color: btnVis.checked ? Theme.success : Theme.borderMedium
                                        border.width: 1
                                        radius: Theme.radiusSmall
                                    }
                                    onClicked: engineBridge.setSelectedVisible(!engineBridge.selectedVisible)
                                }

                                Item { Layout.fillWidth: true }

                                // 聚焦按钮
                                Button {
                                    id: btnFocusNode
                                    text: qsTr("🎯 聚焦此节点")
                                    font.pixelSize: 11
                                    contentItem: Text {
                                        text: btnFocusNode.text
                                        color: btnFocusNode.hovered ? "#ffffff" : Theme.textSecondary
                                        font: btnFocusNode.font
                                        horizontalAlignment: Text.AlignHCenter
                                        verticalAlignment: Text.AlignVCenter
                                    }
                                    background: Rectangle {
                                        implicitWidth: 96
                                        implicitHeight: 26
                                        color: btnFocusNode.hovered ? Theme.bgCardHover : Theme.bgInput
                                        border.color: Theme.borderMedium
                                        border.width: 1
                                        radius: Theme.radiusSmall
                                    }
                                    onClicked: engineBridge.focusTile(engineBridge.selectedTileId)
                                }
                            }
                        }
                    }

                    // --- 卡片 2: 空间几何变换 (TRS) ---
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: colTrs.implicitHeight + 24
                        color: Theme.bgCard
                        border.color: Theme.borderMedium
                        border.width: 1
                        radius: Theme.radiusMedium

                        ColumnLayout {
                            id: colTrs
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            RowLayout {
                                Text {
                                    text: qsTr("局部几何变换 (TRS)")
                                    color: Theme.textPrimary
                                    font.bold: true
                                    font.pixelSize: 12
                                }
                                Item { Layout.fillWidth: true }
                                Text {
                                    text: "f64 Matrix"
                                    color: Theme.textMuted
                                    font.pixelSize: 10
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                height: 1
                                color: Theme.borderSubtle
                            }

                            // --- Translation 平移 (米) ---
                            Text {
                                text: qsTr("平移 (Translation - 米)")
                                color: Theme.textMuted
                                font.pixelSize: 11
                            }

                            RowLayout {
                                spacing: 6

                                // X
                                Rectangle {
                                    Layout.fillWidth: true; height: 26; color: Theme.bgInput
                                    border.color: txField.activeFocus ? Theme.axisX : Theme.borderMedium
                                    border.width: 1; radius: Theme.radiusSmall
                                    RowLayout {
                                        anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 4; spacing: 4
                                        Text { text: "X"; color: Theme.axisX; font.bold: true; font.pixelSize: 11 }
                                        TextInput {
                                            id: txField; Layout.fillWidth: true; text: engineBridge.posX.toFixed(3)
                                            color: Theme.textPrimary; font.pixelSize: 11; selectByMouse: true
                                            onAccepted: root.commitTrs()
                                        }
                                    }
                                }
                                // Y
                                Rectangle {
                                    Layout.fillWidth: true; height: 26; color: Theme.bgInput
                                    border.color: tyField.activeFocus ? Theme.axisY : Theme.borderMedium
                                    border.width: 1; radius: Theme.radiusSmall
                                    RowLayout {
                                        anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 4; spacing: 4
                                        Text { text: "Y"; color: Theme.axisY; font.bold: true; font.pixelSize: 11 }
                                        TextInput {
                                            id: tyField; Layout.fillWidth: true; text: engineBridge.posY.toFixed(3)
                                            color: Theme.textPrimary; font.pixelSize: 11; selectByMouse: true
                                            onAccepted: root.commitTrs()
                                        }
                                    }
                                }
                                // Z
                                Rectangle {
                                    Layout.fillWidth: true; height: 26; color: Theme.bgInput
                                    border.color: tzField.activeFocus ? Theme.axisZ : Theme.borderMedium
                                    border.width: 1; radius: Theme.radiusSmall
                                    RowLayout {
                                        anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 4; spacing: 4
                                        Text { text: "Z"; color: Theme.axisZ; font.bold: true; font.pixelSize: 11 }
                                        TextInput {
                                            id: tzField; Layout.fillWidth: true; text: engineBridge.posZ.toFixed(3)
                                            color: Theme.textPrimary; font.pixelSize: 11; selectByMouse: true
                                            onAccepted: root.commitTrs()
                                        }
                                    }
                                }
                            }

                            // --- Rotation 旋转 (度) ---
                            Text {
                                text: qsTr("旋转 (Rotation - 欧拉角 °)")
                                color: Theme.textMuted
                                font.pixelSize: 11
                            }

                            RowLayout {
                                spacing: 6

                                // RX
                                Rectangle {
                                    Layout.fillWidth: true; height: 26; color: Theme.bgInput
                                    border.color: rxField.activeFocus ? Theme.axisX : Theme.borderMedium
                                    border.width: 1; radius: Theme.radiusSmall
                                    RowLayout {
                                        anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 4; spacing: 4
                                        Text { text: "X°"; color: Theme.axisX; font.bold: true; font.pixelSize: 11 }
                                        TextInput {
                                            id: rxField; Layout.fillWidth: true; text: engineBridge.rotX.toFixed(2)
                                            color: Theme.textPrimary; font.pixelSize: 11; selectByMouse: true
                                            onAccepted: root.commitTrs()
                                        }
                                    }
                                }
                                // RY
                                Rectangle {
                                    Layout.fillWidth: true; height: 26; color: Theme.bgInput
                                    border.color: ryField.activeFocus ? Theme.axisY : Theme.borderMedium
                                    border.width: 1; radius: Theme.radiusSmall
                                    RowLayout {
                                        anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 4; spacing: 4
                                        Text { text: "Y°"; color: Theme.axisY; font.bold: true; font.pixelSize: 11 }
                                        TextInput {
                                            id: ryField; Layout.fillWidth: true; text: engineBridge.rotY.toFixed(2)
                                            color: Theme.textPrimary; font.pixelSize: 11; selectByMouse: true
                                            onAccepted: root.commitTrs()
                                        }
                                    }
                                }
                                // RZ
                                Rectangle {
                                    Layout.fillWidth: true; height: 26; color: Theme.bgInput
                                    border.color: rzField.activeFocus ? Theme.axisZ : Theme.borderMedium
                                    border.width: 1; radius: Theme.radiusSmall
                                    RowLayout {
                                        anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 4; spacing: 4
                                        Text { text: "Z°"; color: Theme.axisZ; font.bold: true; font.pixelSize: 11 }
                                        TextInput {
                                            id: rzField; Layout.fillWidth: true; text: engineBridge.rotZ.toFixed(2)
                                            color: Theme.textPrimary; font.pixelSize: 11; selectByMouse: true
                                            onAccepted: root.commitTrs()
                                        }
                                    }
                                }
                            }

                            // --- Scale 缩放 ---
                            Text {
                                text: qsTr("缩放 (Scale - 轴向倍率)")
                                color: Theme.textMuted
                                font.pixelSize: 11
                            }

                            RowLayout {
                                spacing: 6

                                // SX
                                Rectangle {
                                    Layout.fillWidth: true; height: 26; color: Theme.bgInput
                                    border.color: sxField.activeFocus ? Theme.axisX : Theme.borderMedium
                                    border.width: 1; radius: Theme.radiusSmall
                                    RowLayout {
                                        anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 4; spacing: 4
                                        Text { text: "SX"; color: Theme.axisX; font.bold: true; font.pixelSize: 11 }
                                        TextInput {
                                            id: sxField; Layout.fillWidth: true; text: engineBridge.scaleX.toFixed(3)
                                            color: Theme.textPrimary; font.pixelSize: 11; selectByMouse: true
                                            onAccepted: root.commitTrs()
                                        }
                                    }
                                }
                                // SY
                                Rectangle {
                                    Layout.fillWidth: true; height: 26; color: Theme.bgInput
                                    border.color: syField.activeFocus ? Theme.axisY : Theme.borderMedium
                                    border.width: 1; radius: Theme.radiusSmall
                                    RowLayout {
                                        anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 4; spacing: 4
                                        Text { text: "SY"; color: Theme.axisY; font.bold: true; font.pixelSize: 11 }
                                        TextInput {
                                            id: syField; Layout.fillWidth: true; text: engineBridge.scaleY.toFixed(3)
                                            color: Theme.textPrimary; font.pixelSize: 11; selectByMouse: true
                                            onAccepted: root.commitTrs()
                                        }
                                    }
                                }
                                // SZ
                                Rectangle {
                                    Layout.fillWidth: true; height: 26; color: Theme.bgInput
                                    border.color: szField.activeFocus ? Theme.axisZ : Theme.borderMedium
                                    border.width: 1; radius: Theme.radiusSmall
                                    RowLayout {
                                        anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 4; spacing: 4
                                        Text { text: "SZ"; color: Theme.axisZ; font.bold: true; font.pixelSize: 11 }
                                        TextInput {
                                            id: szField; Layout.fillWidth: true; text: engineBridge.scaleZ.toFixed(3)
                                            color: Theme.textPrimary; font.pixelSize: 11; selectByMouse: true
                                            onAccepted: root.commitTrs()
                                        }
                                    }
                                }
                            }

                            Rectangle { width: 1; height: 2 }

                            // 提交按钮
                            Button {
                                id: btnApply
                                text: qsTr("应用几何变换 (Apply TRS)")
                                font.pixelSize: 12
                                font.bold: true
                                Layout.fillWidth: true

                                contentItem: Text {
                                    text: btnApply.text
                                    color: "#ffffff"
                                    font: btnApply.font
                                    horizontalAlignment: Text.AlignHCenter
                                    verticalAlignment: Text.AlignVCenter
                                }
                                background: Rectangle {
                                    implicitHeight: 30
                                    color: btnApply.pressed ? Theme.accentHover : (btnApply.hovered ? "#0284c7" : Theme.accent)
                                    radius: Theme.radiusSmall
                                }
                                onClicked: root.commitTrs()
                            }
                        }
                    }

                    // --- 卡片 3: 瓦片元数据与拓扑 ---
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: colMeta.implicitHeight + 24
                        color: Theme.bgCard
                        border.color: Theme.borderMedium
                        border.width: 1
                        radius: Theme.radiusMedium

                        ColumnLayout {
                            id: colMeta
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 8

                            Text {
                                text: qsTr("元数据与几何拓扑 (Metadata)")
                                color: Theme.textPrimary
                                font.bold: true
                                font.pixelSize: 12
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                height: 1
                                color: Theme.borderSubtle
                            }

                            Text {
                                text: engineBridge.selectedMetadata
                                color: Theme.textSecondary
                                font.pixelSize: 11
                                font.family: "Consolas, Monaco, monospace"
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }
                        }
                    }
                }
            }
        }
    }
}
