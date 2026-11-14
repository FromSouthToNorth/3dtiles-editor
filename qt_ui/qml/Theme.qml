pragma Singleton
import QtQuick

QtObject {
    id: theme

    // Background Layers
    readonly property color bgApp: "#0b0f19"
    readonly property color bgHeader: "#111827"
    readonly property color bgPanel: "#131b29"
    readonly property color bgPanelHeader: "#182234"
    readonly property color bgCard: "#192233"
    readonly property color bgCardHover: "#202c42"
    readonly property color bgInput: "#151e2c"
    readonly property color bgInputHover: "#1b2638"
    readonly property color bgSelection: "#1d3a63"
    readonly property color bgConsole: "#090d15"

    // Borders
    readonly property color borderSubtle: "#1f2a3d"
    readonly property color borderMedium: "#2b3b55"
    readonly property color borderFocus: "#38bdf8"

    // Semantic 3D Axis Colors (Blender/Unreal Standard)
    readonly property color axisX: "#ef4444"
    readonly property color axisXBg: "#33141a"
    readonly property color axisY: "#22c55e"
    readonly property color axisYBg: "#12331c"
    readonly property color axisZ: "#3b82f6"
    readonly property color axisZBg: "#142542"

    // Accent & Highlights
    readonly property color accent: "#38bdf8"
    readonly property color accentHover: "#0ea5e9"
    readonly property color accentGlow: "#00f0ff"
    readonly property color accentDim: "#173450"

    // Badges & Node Types
    readonly property color badgeTileset: "#a855f7"
    readonly property color badgeTilesetBg: "#2d1645"
    readonly property color badgeMesh: "#06b6d4"
    readonly property color badgeMeshBg: "#0f323e"
    readonly property color badgeEmpty: "#64748b"
    readonly property color badgeEmptyBg: "#1e293b"

    // Status Colors
    readonly property color success: "#10b981"
    readonly property color successBg: "#0d3124"
    readonly property color warning: "#f59e0b"
    readonly property color warningBg: "#38250b"
    readonly property color error: "#f43f5e"
    readonly property color errorBg: "#3b111a"

    // Text Colors
    readonly property color textPrimary: "#f8fafc"
    readonly property color textSecondary: "#cbd5e1"
    readonly property color textMuted: "#94a3b8"
    readonly property color textDisabled: "#475569"

    // Standard Metrics
    readonly property int radiusSmall: 3
    readonly property int radiusMedium: 5
    readonly property int radiusLarge: 8
}
