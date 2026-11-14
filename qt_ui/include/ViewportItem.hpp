#pragma once

#include <QPointer>
#include <QQuickItem>
#include <QWindow>

class EngineBridge;

class ViewportItem : public QQuickItem {
    Q_OBJECT
    Q_PROPERTY(EngineBridge* engine READ engine WRITE setEngine NOTIFY engineChanged)

public:
    explicit ViewportItem(QQuickItem* parent = nullptr);
    ~ViewportItem() override;

    EngineBridge* engine() const { return m_engine; }
    void setEngine(EngineBridge* engine);

signals:
    void engineChanged();

protected:
    void itemChange(ItemChange change, const ItemChangeData& value) override;
    void geometryChange(const QRectF& newGeo, const QRectF& oldGeo) override;

private slots:
    void syncNativeWindow();

private:
    void ensureChildWindow(QQuickWindow* host);

    QPointer<EngineBridge> m_engine;
    QWindow* m_childWindow = nullptr;
    bool m_attached = false;
};
