#include "EngineBridge.hpp"
#include "tiles_ffi.h"

#include <QFileDialog>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

#if defined(Q_OS_LINUX)
#include <QtGui/qnativeinterface.h>
#endif

EngineBridge::EngineBridge(QObject* parent)
    : QObject(parent), m_treeModel(this) {
    m_handle = engine_create();
    m_treeModel.setHandle(m_handle);

    connect(&m_pollTimer, &QTimer::timeout, this, &EngineBridge::pollEngineEvents);
    m_pollTimer.start(16);
}

EngineBridge::~EngineBridge() {
    m_pollTimer.stop();
    if (m_handle) {
        engine_destroy(m_handle);
        m_handle = nullptr;
    }
}

bool EngineBridge::attachViewport(QWindow* childWindow, uint32_t width, uint32_t height) {
    if (!m_handle || !childWindow) {
        return false;
    }
    NativeWindowInfo info{};
    info.width = width;
    info.height = height;

#if defined(Q_OS_WIN)
    info.backend = 1;
    info.window_id = static_cast<uint64_t>(childWindow->winId());
#elif defined(Q_OS_MACOS)
    info.backend = 2;
    info.window_id = static_cast<uint64_t>(childWindow->winId());
#elif defined(Q_OS_LINUX)
    info.backend = 3;
    info.window_id = static_cast<uint64_t>(childWindow->winId());
    if (auto* x11 = qGuiApp->nativeInterface<QNativeInterface::QX11Application>()) {
        info.display_ptr = x11->display();
        info.x11_screen = 0;
    }
#endif

    const bool ok = engine_attach_window(m_handle, info);
    if (ok && !m_pendingOpenPath.isEmpty()) {
        const QByteArray utf8 = m_pendingOpenPath.toUtf8();
        engine_open_tileset(m_handle, utf8.constData());
        m_pendingOpenPath.clear();
    }
    return ok;
}

void EngineBridge::resizeViewport(uint32_t width, uint32_t height) {
    if (m_handle) {
        engine_resize(m_handle, width, height);
    }
}

void EngineBridge::orbitCamera(double dx, double dy) {
    if (m_handle) {
        engine_camera_orbit(m_handle, dx, dy);
    }
}

void EngineBridge::panCamera(double dx, double dy) {
    if (m_handle) {
        engine_camera_pan(m_handle, dx, dy);
    }
}

void EngineBridge::zoomCamera(double deltaSteps) {
    if (m_handle) {
        engine_camera_zoom(m_handle, deltaSteps);
    }
}

void EngineBridge::pickAt(double physX, double physY) {
    if (m_handle) {
        engine_pick_at(m_handle, physX, physY);
    }
}

void EngineBridge::openDialog() {
    const QString path = QFileDialog::getOpenFileName(
        nullptr,
        tr("打开 3D Tiles 数据集"),
        QString(),
        tr("3D Tiles (tileset.json);;JSON (*.json)"));
    if (!path.isEmpty()) {
        openPath(path);
    }
}

void EngineBridge::openPath(const QString& path) {
    if (path.isEmpty()) {
        return;
    }
    if (!m_handle) {
        return;
    }
    const QByteArray utf8 = path.toUtf8();
    if (!engine_open_tileset(m_handle, utf8.constData())) {
        // 若视口尚未挂载，暂存待附加后立即打开
        m_pendingOpenPath = path;
    }
}

void EngineBridge::saveCurrent() {
    if (m_handle) {
        engine_save_tileset(m_handle, nullptr);
    }
}

void EngineBridge::saveAsDialog() {
    if (!m_handle) {
        return;
    }
    const QString path = QFileDialog::getSaveFileName(
        nullptr,
        tr("另存 tileset.json"),
        QStringLiteral("tileset.json"),
        tr("3D Tiles (*.json)"));
    if (!path.isEmpty()) {
        const QByteArray utf8 = path.toUtf8();
        engine_save_tileset(m_handle, utf8.constData());
    }
}

void EngineBridge::selectTile(qulonglong tileId) {
    if (m_handle) {
        engine_select_tile(m_handle, static_cast<uint64_t>(tileId));
    }
}

void EngineBridge::focusTile(qulonglong tileId) {
    if (m_handle) {
        engine_focus_tile(m_handle, static_cast<uint64_t>(tileId));
    }
}

void EngineBridge::setMaxSse(double sse) {
    if (m_handle) {
        engine_set_max_sse(m_handle, sse);
    }
}

void EngineBridge::setShowAllBBoxes(bool show) {
    if (m_handle) {
        engine_set_show_all_bboxes(m_handle, show);
    }
}

void EngineBridge::setSelectedVisible(bool visible) {
    if (m_handle && m_selectedTileId != 0) {
        engine_set_tile_visible(m_handle, m_selectedTileId, visible);
    }
}

void EngineBridge::applySelectedTrs(
    double tx, double ty, double tz,
    double rx, double ry, double rz,
    double sx, double sy, double sz) {
    if (!m_handle || m_selectedTileId == 0) {
        return;
    }
    TrsC trs{tx, ty, tz, rx, ry, rz, sx, sy, sz};
    engine_set_tile_trs(m_handle, m_selectedTileId, trs);
}

void EngineBridge::undo() {
    if (m_handle) {
        engine_undo(m_handle);
    }
}

void EngineBridge::redo() {
    if (m_handle) {
        engine_redo(m_handle);
    }
}

void EngineBridge::pollEngineEvents() {
    if (!m_handle) {
        return;
    }
    for (int i = 0; i < 64; ++i) {
        char* raw = engine_poll_event_json(m_handle);
        if (!raw) {
            break;
        }
        const auto doc = QJsonDocument::fromJson(QByteArray(raw));
        engine_free_string(raw);
        if (!doc.isObject()) {
            continue;
        }
        const QJsonObject obj = doc.object();
        const QString type = obj.value("type").toString();

        if (type == QLatin1String("log")) {
            m_lastLog = QStringLiteral("[%1] %2")
                            .arg(obj.value("level").toString().toUpper(),
                                 obj.value("message").toString());
            emit logUpdated();
        } else if (type == QLatin1String("progress")) {
            m_isLoading = obj.value("active").toBool();
            m_loadProgress = obj.value("progress").toDouble();
            emit progressUpdated();
        } else if (type == QLatin1String("tileset_opened")) {
            m_tilesetLoaded = true;
            m_currentPath = obj.value("path").toString();
            if (!m_errorMessage.isEmpty()) {
                m_errorMessage.clear();
                emit errorRaised();
            }
            m_treeModel.resetWithRoot();
            emit tilesetStateChanged();
        } else if (type == QLatin1String("external_tileset_merged")) {
            const uint64_t pid = static_cast<uint64_t>(obj.value("parent_id").toInteger());
            m_treeModel.refreshExternalParent(pid);
        } else if (type == QLatin1String("selection_changed")) {
            const QJsonValue sum = obj.value("summary");
            if (sum.isObject()) {
                applySummaryJson(sum.toObject());
            } else {
                m_selectedTileId = 0;
                m_selectedLabel.clear();
                m_selectedMetadata.clear();
                emit selectionChanged();
            }
        } else if (type == QLatin1String("tile_updated")) {
            const QJsonObject sum = obj.value("summary").toObject();
            const uint64_t tid = static_cast<uint64_t>(sum.value("id").toInteger());
            m_treeModel.updateTileVisible(tid, sum.value("visible").toBool(true));
            if (tid == m_selectedTileId) {
                applySummaryJson(sum);
            }
        } else if (type == QLatin1String("stats")) {
            m_statsText = QStringLiteral("%1x%2 | FPS:%3 | Draw:%4 | Tiles:%5 | VRAM:%6MB | IO:%7 | N:%8 (V:%9)")
                              .arg(obj.value("vw").toInt())
                              .arg(obj.value("vh").toInt())
                              .arg(obj.value("fps").toDouble(), 0, 'f', 0)
                              .arg(obj.value("draw_calls").toInt())
                              .arg(obj.value("rendered_tiles").toInt())
                              .arg(obj.value("gpu_mb").toDouble(), 0, 'f', 1)
                              .arg(obj.value("pending_io").toInteger())
                              .arg(obj.value("total_nodes").toInteger())
                              .arg(obj.value("visited").toInteger());
            emit statsUpdated();
        } else if (type == QLatin1String("error")) {
            m_errorMessage = obj.value("message").toString();
            emit errorRaised();
        }
    }
}

void EngineBridge::applySummaryJson(const QJsonObject& obj) {
    m_selectedTileId = static_cast<uint64_t>(obj.value("id").toInteger());
    m_selectedLabel = obj.value("label").toString();
    m_selectedVisible = obj.value("visible").toBool(true);

    const QJsonArray c = obj.value("bv_center").toArray();
    const QString centerStr = c.size() == 3
        ? QStringLiteral("[%1, %2, %3]")
              .arg(c[0].toDouble(), 0, 'f', 2)
              .arg(c[1].toDouble(), 0, 'f', 2)
              .arg(c[2].toDouble(), 0, 'f', 2)
        : QStringLiteral("-");

    m_selectedMetadata = QStringLiteral(
        "Refine: %1  |  BV: %2 (r = %3 m)\nCenter: %4\nGeometricError: %5 m")
        .arg(obj.value("refine").toString(),
             obj.value("bv_kind").toString(),
             QString::number(obj.value("bv_radius").toDouble(), 'f', 2),
             centerStr,
             QString::number(obj.value("geometric_error").toDouble(), 'f', 3));

    const QJsonObject trs = obj.value("trs").toObject();
    const QJsonArray t = trs.value("translation").toArray();
    const QJsonArray r = trs.value("rotation_deg").toArray();
    const QJsonArray s = trs.value("scale").toArray();
    for (int i = 0; i < 3; ++i) {
        m_pos[i] = t.size() == 3 ? t[i].toDouble() : 0.0;
        m_rot[i] = r.size() == 3 ? r[i].toDouble() : 0.0;
        m_scale[i] = s.size() == 3 ? s[i].toDouble() : 1.0;
    }
    emit selectionChanged();
}
