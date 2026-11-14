import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import EditorUI

Rectangle {
    id: root
    color: Theme.bgPanel
    border.color: Theme.borderSubtle
    border.width: 1

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // --- 面板标题栏 ---
        Rectangle {
            Layout.fillWidth: true
            height: 38
            color: Theme.bgPanelHeader

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 12

                Text {
                    text: qsTr("场景层级大纲")
                    color: Theme.textPrimary
                    font.bold: true
                    font.pixelSize: 13
                }

                Item { Layout.fillWidth: true }

                Rectangle {
                    implicitWidth: countText.implicitWidth + 12
                    implicitHeight: 20
                    radius: 10
                    color: Theme.bgCard
                    border.color: Theme.borderMedium
                    border.width: 1

                    Text {
                        id: countText
                        anchors.centerIn: parent
                        text: engineBridge.tilesetLoaded ? "40,120 节点" : "无数据"
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

        // --- 快速检索过滤栏 ---
        Rectangle {
            Layout.fillWidth: true
            height: 38
            color: Theme.bgPanel

            Rectangle {
                anchors.fill: parent
                anchors.margins: 6
                color: Theme.bgInput
                border.color: searchField.activeFocus ? Theme.borderFocus : Theme.borderMedium
                border.width: 1
                radius: Theme.radiusSmall

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    anchors.rightMargin: 6
                    spacing: 6

                    Text {
                        text: "🔍"
                        color: Theme.textMuted
                        font.pixelSize: 11
                    }

                    TextInput {
                        id: searchField
                        Layout.fillWidth: true
                        color: Theme.textPrimary
                        font.pixelSize: 12
                        selectByMouse: true
                        clip: true

                        Text {
                            text: qsTr("搜索瓦片 ID / 路径...")
                            color: Theme.textDisabled
                            font: searchField.font
                            visible: !searchField.text && !searchField.activeFocus
                            anchors.verticalCenter: parent.verticalCenter
                        }
                    }

                    Text {
                        visible: searchField.text.length > 0
                        text: "✕"
                        color: Theme.textMuted
                        font.pixelSize: 11
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: searchField.text = ""
                        }
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

        // --- 树结构视图 ---
        TreeView {
            id: tree
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: engineBridge.tileTreeModel
            selectionModel: ItemSelectionModel { model: tree.model }

            ScrollBar.vertical: ScrollBar {
                policy: ScrollBar.AsNeeded
                contentItem: Rectangle {
                    implicitWidth: 6
                    radius: 3
                    color: Theme.borderMedium
                }
            }

            delegate: TreeViewDelegate {
                id: nodeDelegate
                implicitWidth: tree.width
                implicitHeight: 28

                readonly property bool isSelected: model.tileId === engineBridge.selectedTileId
                readonly property bool isTilesetNode: model.label.indexOf("tileset") !== -1 || depth === 0
                readonly property bool hasMesh: model.hasContent

                background: Rectangle {
                    radius: Theme.radiusSmall
                    color: nodeDelegate.isSelected
                        ? Theme.bgSelection
                        : (nodeDelegate.hovered ? Theme.bgCardHover : "transparent")

                    // 选中状态左侧强调发光线
                    Rectangle {
                        visible: nodeDelegate.isSelected
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        width: 3
                        color: Theme.accent
                        radius: 1
                    }
                }

                contentItem: RowLayout {
                    spacing: 6

                    // 节点类型彩色徽标 (Type Badge)
                    Rectangle {
                        implicitWidth: badgeLabel.implicitWidth + 8
                        implicitHeight: 18
                        radius: 3
                        color: nodeDelegate.isTilesetNode
                            ? Theme.badgeTilesetBg
                            : (nodeDelegate.hasMesh ? Theme.badgeMeshBg : Theme.badgeEmptyBg)
                        border.color: nodeDelegate.isTilesetNode
                            ? Theme.badgeTileset
                            : (nodeDelegate.hasMesh ? Theme.badgeMesh : Theme.badgeEmpty)
                        border.width: 1

                        Text {
                            id: badgeLabel
                            anchors.centerIn: parent
                            text: nodeDelegate.isTilesetNode ? "TS" : (nodeDelegate.hasMesh ? "GLB" : "IDX")
                            color: nodeDelegate.isTilesetNode
                                ? Theme.badgeTileset
                                : (nodeDelegate.hasMesh ? Theme.badgeMesh : Theme.textMuted)
                            font.pixelSize: 9
                            font.bold: true
                        }
                    }

                    // 瓦片名称 / 标签
                    Text {
                        text: model.label
                        color: model.tileVisible
                            ? (nodeDelegate.isSelected ? "#ffffff" : Theme.textPrimary)
                            : Theme.textDisabled
                        font.pixelSize: 12
                        font.bold: nodeDelegate.isSelected
                        elide: Text.ElideMiddle
                        Layout.fillWidth: true
                    }

                    // 几何误差微芯片
                    Text {
                        visible: model.geometricError > 0
                        text: model.geometricError.toFixed(1) + "m"
                        color: Theme.textMuted
                        font.pixelSize: 10
                        Layout.rightMargin: 8
                    }
                }

                onClicked: engineBridge.selectTile(model.tileId)
                onDoubleClicked: engineBridge.focusTile(model.tileId)
            }
        }
    }
}
