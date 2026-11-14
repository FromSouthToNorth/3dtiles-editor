import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import EditorUI

Rectangle {
    id: root
    color: Theme.bgConsole
    border.color: Theme.borderSubtle
    border.width: 1

    Rectangle {
        anchors.top: parent.top
        width: parent.width
        height: 1
        color: Theme.borderMedium
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        spacing: 10

        // 终端图标标识
        Rectangle {
            implicitWidth: 22
            implicitHeight: 20
            color: Theme.bgCard
            border.color: Theme.borderMedium
            border.width: 1
            radius: Theme.radiusSmall

            Text {
                anchors.centerIn: parent
                text: ">_"
                color: Theme.accent
                font.bold: true
                font.pixelSize: 10
            }
        }

        // 日志级别徽标 (INFO / ERROR)
        Rectangle {
            implicitWidth: lvlText.implicitWidth + 8
            implicitHeight: 20
            radius: Theme.radiusSmall
            color: engineBridge.errorMessage.length > 0 ? Theme.errorBg : Theme.bgCard
            border.color: engineBridge.errorMessage.length > 0 ? Theme.error : Theme.borderMedium
            border.width: 1

            Text {
                id: lvlText
                anchors.centerIn: parent
                text: engineBridge.errorMessage.length > 0 ? "ERROR" : "INFO"
                color: engineBridge.errorMessage.length > 0 ? Theme.error : Theme.accent
                font.pixelSize: 9
                font.bold: true
            }
        }

        // 日志正文
        Text {
            text: engineBridge.errorMessage.length > 0
                ? engineBridge.errorMessage
                : (engineBridge.lastLog.length > 0 ? engineBridge.lastLog : "就绪，等待打开场景或载入数据...")
            color: engineBridge.errorMessage.length > 0 ? Theme.error : Theme.textSecondary
            elide: Text.ElideRight
            Layout.fillWidth: true
            font.pixelSize: 11
            font.family: "Consolas, Monaco, monospace"
        }

        // 后台任务加载进度条
        RowLayout {
            visible: engineBridge.isLoading
            spacing: 6

            Text {
                text: qsTr("加载切片中...")
                color: Theme.textMuted
                font.pixelSize: 11
            }

            ProgressBar {
                value: engineBridge.loadProgress
                implicitWidth: 140
                implicitHeight: 6

                background: Rectangle {
                    implicitWidth: 140
                    implicitHeight: 6
                    color: Theme.bgInput
                    radius: 3
                }

                contentItem: Item {
                    implicitWidth: 140
                    implicitHeight: 6

                    Rectangle {
                        width: parent.width * engineBridge.loadProgress
                        height: parent.height
                        radius: 3
                        color: Theme.accent
                    }
                }
            }
        }
    }
}
