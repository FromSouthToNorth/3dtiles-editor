#include "EngineBridge.hpp"
#include "ViewportItem.hpp"

#include <QApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>

int main(int argc, char* argv[]) {
#if defined(Q_OS_LINUX)
    // MVP 阶段在 Wayland 会话下默认回退 XWayland，保证原生子 QWindow 句柄可用
    if (qEnvironmentVariableIsSet("WAYLAND_DISPLAY") && qEnvironmentVariableIsEmpty("QT_QPA_PLATFORM")) {
        qputenv("QT_QPA_PLATFORM", "xcb");
    }
#endif

    QApplication app(argc, argv);
    app.setApplicationName(QStringLiteral("3dtiles-editor"));
    app.setOrganizationName(QStringLiteral("3dtiles-editor"));
    QQuickStyle::setStyle(QStringLiteral("Basic"));

    qmlRegisterType<ViewportItem>("EditorUI", 1, 0, "ViewportItem");
    qmlRegisterUncreatableType<EngineBridge>("EditorUI", 1, 0, "EngineBridge", QStringLiteral("Owned by C++"));

    EngineBridge bridge;
    if (argc > 1) {
        bridge.openPath(QString::fromLocal8Bit(argv[1]));
    }

    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("engineBridge"), &bridge);
    engine.loadFromModule("EditorUI", "Main");
    if (engine.rootObjects().isEmpty()) {
        return -1;
    }
    return app.exec();
}
