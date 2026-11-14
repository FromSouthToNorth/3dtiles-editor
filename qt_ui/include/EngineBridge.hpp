#pragma once

#include "TileTreeModel.hpp"
#include <QObject>
#include <QTimer>
#include <QWindow>
#include <cstdint>

struct EngineHandle;

class EngineBridge : public QObject {
    Q_OBJECT
    Q_PROPERTY(TileTreeModel* tileTreeModel READ tileTreeModel CONSTANT)
    Q_PROPERTY(bool tilesetLoaded READ tilesetLoaded NOTIFY tilesetStateChanged)
    Q_PROPERTY(QString currentPath READ currentPath NOTIFY tilesetStateChanged)
    Q_PROPERTY(qulonglong selectedTileId READ selectedTileId NOTIFY selectionChanged)
    Q_PROPERTY(QString selectedLabel READ selectedLabel NOTIFY selectionChanged)
    Q_PROPERTY(QString selectedMetadata READ selectedMetadata NOTIFY selectionChanged)
    Q_PROPERTY(bool selectedVisible READ selectedVisible NOTIFY selectionChanged)
    Q_PROPERTY(double posX READ posX NOTIFY selectionChanged)
    Q_PROPERTY(double posY READ posY NOTIFY selectionChanged)
    Q_PROPERTY(double posZ READ posZ NOTIFY selectionChanged)
    Q_PROPERTY(double rotX READ rotX NOTIFY selectionChanged)
    Q_PROPERTY(double rotY READ rotY NOTIFY selectionChanged)
    Q_PROPERTY(double rotZ READ rotZ NOTIFY selectionChanged)
    Q_PROPERTY(double scaleX READ scaleX NOTIFY selectionChanged)
    Q_PROPERTY(double scaleY READ scaleY NOTIFY selectionChanged)
    Q_PROPERTY(double scaleZ READ scaleZ NOTIFY selectionChanged)
    Q_PROPERTY(QString lastLog READ lastLog NOTIFY logUpdated)
    Q_PROPERTY(QString statsText READ statsText NOTIFY statsUpdated)
    Q_PROPERTY(bool isLoading READ isLoading NOTIFY progressUpdated)
    Q_PROPERTY(double loadProgress READ loadProgress NOTIFY progressUpdated)
    Q_PROPERTY(QString errorMessage READ errorMessage NOTIFY errorRaised)

public:
    explicit EngineBridge(QObject* parent = nullptr);
    ~EngineBridge() override;

    TileTreeModel* tileTreeModel() { return &m_treeModel; }

    bool attachViewport(QWindow* childWindow, uint32_t width, uint32_t height);
    void resizeViewport(uint32_t width, uint32_t height);
    void orbitCamera(double dx, double dy);
    void panCamera(double dx, double dy);
    void zoomCamera(double deltaSteps);
    void pickAt(double physX, double physY);

    Q_INVOKABLE void openDialog();
    Q_INVOKABLE void openPath(const QString& path);
    Q_INVOKABLE void saveCurrent();
    Q_INVOKABLE void saveAsDialog();
    Q_INVOKABLE void selectTile(qulonglong tileId);
    Q_INVOKABLE void focusTile(qulonglong tileId);
    Q_INVOKABLE void setMaxSse(double sse);
    Q_INVOKABLE void setShowAllBBoxes(bool show);
    Q_INVOKABLE void setSelectedVisible(bool visible);
    Q_INVOKABLE void applySelectedTrs(
        double tx, double ty, double tz,
        double rx, double ry, double rz,
        double sx, double sy, double sz);
    Q_INVOKABLE void undo();
    Q_INVOKABLE void redo();

    bool tilesetLoaded() const { return m_tilesetLoaded; }
    QString currentPath() const { return m_currentPath; }
    qulonglong selectedTileId() const { return m_selectedTileId; }
    QString selectedLabel() const { return m_selectedLabel; }
    QString selectedMetadata() const { return m_selectedMetadata; }
    bool selectedVisible() const { return m_selectedVisible; }
    double posX() const { return m_pos[0]; }
    double posY() const { return m_pos[1]; }
    double posZ() const { return m_pos[2]; }
    double rotX() const { return m_rot[0]; }
    double rotY() const { return m_rot[1]; }
    double rotZ() const { return m_rot[2]; }
    double scaleX() const { return m_scale[0]; }
    double scaleY() const { return m_scale[1]; }
    double scaleZ() const { return m_scale[2]; }
    QString lastLog() const { return m_lastLog; }
    QString statsText() const { return m_statsText; }
    bool isLoading() const { return m_isLoading; }
    double loadProgress() const { return m_loadProgress; }
    QString errorMessage() const { return m_errorMessage; }

signals:
    void tilesetStateChanged();
    void selectionChanged();
    void logUpdated();
    void statsUpdated();
    void progressUpdated();
    void errorRaised();

private slots:
    void pollEngineEvents();

private:
    void applySummaryJson(const class QJsonObject& obj);

    EngineHandle* m_handle = nullptr;
    TileTreeModel m_treeModel;
    QTimer m_pollTimer;
    QString m_pendingOpenPath;

    bool m_tilesetLoaded = false;
    QString m_currentPath;
    uint64_t m_selectedTileId = 0;
    QString m_selectedLabel;
    QString m_selectedMetadata;
    bool m_selectedVisible = true;
    double m_pos[3] = {0.0, 0.0, 0.0};
    double m_rot[3] = {0.0, 0.0, 0.0};
    double m_scale[3] = {1.0, 1.0, 1.0};

    QString m_lastLog = QStringLiteral("就绪");
    QString m_statsText = QStringLiteral("FPS: --");
    bool m_isLoading = false;
    double m_loadProgress = 0.0;
    QString m_errorMessage;
};
