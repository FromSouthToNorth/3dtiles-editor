import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    color: "#151b26"
    border.color: "#243044"
    border.width: 1

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            height: 32
            color: "#1b2332"
            Label {
                anchors.verticalCenter: parent.verticalCenter
                anchors.left: parent.left
                anchors.leftMargin: 10
                text: qsTr("Tileset 层级大纲")
                color: "#e2e8f0"
                font.bold: true
            }
        }

        TreeView {
            id: tree
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: engineBridge.tileTreeModel
            selectionModel: ItemSelectionModel { model: tree.model }

            delegate: TreeViewDelegate {
                implicitWidth: tree.width
                implicitHeight: 26
                background: Rectangle {
                    color: model.tileId === engineBridge.selectedTileId ? "#1e3a5f" : "transparent"
                }
                contentItem: RowLayout {
                    spacing: 6
                    Label {
                        text: model.label
                        color: model.tileVisible ? "#e2e8f0" : "#64748b"
                        elide: Text.ElideMiddle
                        Layout.fillWidth: true
                    }
                    Label {
                        text: model.hasContent ? "●" : "○"
                        color: model.hasContent ? "#38bdf8" : "#475569"
                        font.pixelSize: 10
                    }
                }
                onClicked: engineBridge.selectTile(model.tileId)
                onDoubleClicked: engineBridge.focusTile(model.tileId)
            }
        }
    }
}
