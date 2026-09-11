import QtQuick

// A rounded metadata chip for the task row.
//
// Color is opt-in per device (`colored`), because it is not a free upgrade: a
// hue that a grayscale panel dithers lands as mid-gray mush, which reads worse
// than the plain outlined chip it replaced. On a monochrome display the accent
// is dropped entirely and the pill falls back to a dark outline on white, which
// is the same contrast the rest of the UI is built on.
//
// The fill is a heavily-lightened accent rather than the accent itself: e-ink
// color saturation is low, and dark text on a pale tint survives a partial
// refresh far better than light text on a saturated fill.
Rectangle {
    id: pill

    property alias text: label.text
    // Hue used when `colored` is true; ignored entirely otherwise.
    property color accent: "#303030"
    property bool colored: false
    // Completed rows dim their pills so the row still reads as struck-through.
    property bool muted: false

    readonly property int horizontalPadding: 14

    implicitWidth: label.implicitWidth + 2 * horizontalPadding
    implicitHeight: 36
    radius: height / 2

    color: {
        if (pill.muted) return "#f2f2f2"
        if (!pill.colored) return "white"
        return Qt.rgba(pill.accent.r, pill.accent.g, pill.accent.b, 0.18)
    }

    border.width: 2
    border.color: {
        if (pill.muted) return "#a0a0a0"
        if (!pill.colored) return "#303030"
        return pill.accent
    }

    Text {
        id: label
        anchors.centerIn: parent
        font.pixelSize: 20
        // Darkening the accent keeps the label above the tinted fill at the
        // contrast e-ink needs; the uncolored path is already near-black.
        color: {
            if (pill.muted) return "#808080"
            if (!pill.colored) return "#202020"
            return Qt.darker(pill.accent, 1.6)
        }
    }
}
