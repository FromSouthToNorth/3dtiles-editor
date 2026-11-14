#include "ViewportItem.hpp"
#include "EngineBridge.hpp"

#include <QMouseEvent>
#include <QQuickWindow>
#include <QResizeEvent>
#include <QWheelEvent>

#if defined(Q_OS_WIN)
#include <windows.h>
#endif

namespace {
class ViewportWindow final : public QWindow {
public:
    explicit ViewportWindow(QWindow* parent, EngineBridge* bridge)
        : QWindow(parent), m_bridge(bridge) {
        setSurfaceType(QSurface::VulkanSurface);
        setFlags(Qt::Widget);
    }

    void setBridge(EngineBridge* bridge) { m_bridge = bridge; }

protected:
#if defined(Q_OS_WIN)
    bool nativeEvent(const QByteArray &eventType, void *message, qintptr *result) override {
        MSG* msg = static_cast<MSG*>(message);
        if (msg) {
            if (msg->message == WM_ERASEBKGND) {
                *result = 1;
                return true;
            }
            if (msg->message == WM_PAINT) {
                PAINTSTRUCT ps;
                BeginPaint(reinterpret_cast<HWND>(winId()), &ps);
                EndPaint(reinterpret_cast<HWND>(winId()), &ps);
                *result = 0;
                return true;
            }
        }
        return QWindow::nativeEvent(eventType, message, result);
    }
#endif

    void mousePressEvent(QMouseEvent* ev) override {
        m_lastPos = ev->position();
        m_pressPos = ev->position();
        m_buttons = ev->buttons();
        ev->accept();
    }

    void mouseMoveEvent(QMouseEvent* ev) override {
        if (!m_bridge) return;
        const QPointF cur = ev->position();
        const QPointF delta = cur - m_lastPos;
        m_lastPos = cur;
        if (m_buttons & Qt::LeftButton) {
            m_bridge->orbitCamera(delta.x(), delta.y());
        } else if (m_buttons & (Qt::RightButton | Qt::MiddleButton)) {
            m_bridge->panCamera(delta.x(), delta.y());
        }
        ev->accept();
    }

    void mouseReleaseEvent(QMouseEvent* ev) override {
        if (m_bridge && ev->button() == Qt::LeftButton) {
            const QPointF moved = ev->position() - m_pressPos;
            if (moved.manhattanLength() < 4.0) {
                const qreal dpr = devicePixelRatio();
                m_bridge->pickAt(ev->position().x() * dpr, ev->position().y() * dpr);
            }
        }
        m_buttons = ev->buttons();
        ev->accept();
    }

    void wheelEvent(QWheelEvent* ev) override {
        if (m_bridge) {
            const double steps = ev->angleDelta().y() / 120.0;
            m_bridge->zoomCamera(steps);
        }
        ev->accept();
    }

private:
    EngineBridge* m_bridge = nullptr;
    QPointF m_lastPos;
    QPointF m_pressPos;
    Qt::MouseButtons m_buttons = Qt::NoButton;
};
} // namespace

ViewportItem::ViewportItem(QQuickItem* parent)
    : QQuickItem(parent) {
    setFlag(ItemHasContents, false);
    connect(this, &QQuickItem::xChanged, this, &ViewportItem::syncNativeWindow);
    connect(this, &QQuickItem::yChanged, this, &ViewportItem::syncNativeWindow);
    connect(this, &QQuickItem::visibleChanged, this, &ViewportItem::syncNativeWindow);
}

ViewportItem::~ViewportItem() {
    if (m_childWindow) {
        m_childWindow->setParent(nullptr);
        m_childWindow->deleteLater();
        m_childWindow = nullptr;
    }
}

void ViewportItem::setEngine(EngineBridge* engine) {
    if (m_engine == engine) return;
    m_engine = engine;
    if (auto* vw = static_cast<ViewportWindow*>(m_childWindow)) {
        vw->setBridge(m_engine);
    }
    emit engineChanged();
    syncNativeWindow();
}

void ViewportItem::itemChange(ItemChange change, const ItemChangeData& value) {
    QQuickItem::itemChange(change, value);
    if (change == ItemSceneChange && value.window) {
        ensureChildWindow(value.window);
        syncNativeWindow();
    }
}

void ViewportItem::geometryChange(const QRectF& newGeo, const QRectF& oldGeo) {
    QQuickItem::geometryChange(newGeo, oldGeo);
    syncNativeWindow();
}

void ViewportItem::ensureChildWindow(QQuickWindow* host) {
    if (m_childWindow || !host) return;
    auto* win = new ViewportWindow(host, m_engine);
    win->setVisible(isVisible());
    win->winId();
#if defined(Q_OS_WIN)
    HWND parentHwnd = reinterpret_cast<HWND>(host->winId());
    LONG_PTR parentStyle = GetWindowLongPtr(parentHwnd, GWL_STYLE);
    SetWindowLongPtr(parentHwnd, GWL_STYLE, parentStyle | WS_CLIPCHILDREN);

    HWND childHwnd = reinterpret_cast<HWND>(win->winId());
    LONG_PTR childStyle = GetWindowLongPtr(childHwnd, GWL_STYLE);
    SetWindowLongPtr(childHwnd, GWL_STYLE, childStyle | WS_CLIPSIBLINGS);
    SetClassLongPtr(childHwnd, GCLP_HBRBACKGROUND, (LONG_PTR)NULL);
#endif
    m_childWindow = win;
    connect(host, &QWindow::screenChanged, this, &ViewportItem::syncNativeWindow);
}

void ViewportItem::syncNativeWindow() {
    if (!m_childWindow || !window()) return;
    if (width() < 2.0 || height() < 2.0) {
        m_childWindow->setVisible(false);
        return;
    }
    m_childWindow->setVisible(isVisible());

    const QPointF scenePos = mapToScene(QPointF(0.0, 0.0));
    const int lw = static_cast<int>(width());
    const int lh = static_cast<int>(height());
    m_childWindow->setGeometry(
        static_cast<int>(scenePos.x()),
        static_cast<int>(scenePos.y()),
        lw,
        lh);

    const qreal dpr = window()->effectiveDevicePixelRatio();
    const uint32_t pw = static_cast<uint32_t>(qMax(1.0, lw * dpr));
    const uint32_t ph = static_cast<uint32_t>(qMax(1.0, lh * dpr));
    if (!m_attached && m_engine) {
        m_attached = m_engine->attachViewport(m_childWindow, pw, ph);
    } else if (m_attached && m_engine) {
        m_engine->resizeViewport(pw, ph);
    }
}
