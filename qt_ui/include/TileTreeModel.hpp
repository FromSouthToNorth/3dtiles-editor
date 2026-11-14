#pragma once

#include <QAbstractItemModel>
#include <QHash>
#include <QVector>
#include <cstdint>

struct EngineHandle;

class TileTreeModel : public QAbstractItemModel {
    Q_OBJECT
public:
    enum Roles {
        TileIdRole = Qt::UserRole + 1,
        LabelRole,
        VisibleRole,
        HasContentRole,
        GeometricErrorRole,
    };

    explicit TileTreeModel(QObject* parent = nullptr);
    void setHandle(EngineHandle* handle);

    void resetWithRoot();
    void refreshExternalParent(uint64_t parentTileId);
    void updateTileVisible(uint64_t tileId, bool visible);
    QModelIndex indexForTile(uint64_t tileId) const;

    int rowCount(const QModelIndex& parent = QModelIndex()) const override;
    int columnCount(const QModelIndex& parent = QModelIndex()) const override;
    QModelIndex index(int row, int column, const QModelIndex& parent = QModelIndex()) const override;
    QModelIndex parent(const QModelIndex& child) const override;
    QVariant data(const QModelIndex& index, int role = Qt::DisplayRole) const override;
    bool hasChildren(const QModelIndex& parent = QModelIndex()) const override;
    bool canFetchMore(const QModelIndex& parent) const override;
    void fetchMore(const QModelIndex& parent) override;
    QHash<int, QByteArray> roleNames() const override;

private:
    struct NodeEntry {
        uint64_t tileId = 0;
        QString label;
        bool visible = true;
        bool hasContent = false;
        double geometricError = 0.0;
        bool mayHaveChildren = false;
        bool fetched = false;
        int parentEntryIdx = -1;
        int rowInParent = 0;
        QVector<int> children;
    };

    void appendChildrenFromEngine(int parentEntryIdx, uint64_t parentTileId);

    EngineHandle* m_handle = nullptr;
    QVector<NodeEntry> m_entries;
    QVector<int> m_rootIndices;
    QHash<uint64_t, int> m_tileToEntry;
};
