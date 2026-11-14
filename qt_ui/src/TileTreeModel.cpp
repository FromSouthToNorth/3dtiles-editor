#include "TileTreeModel.hpp"
#include "tiles_ffi.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

TileTreeModel::TileTreeModel(QObject* parent)
    : QAbstractItemModel(parent) {}

void TileTreeModel::setHandle(EngineHandle* handle) {
    m_handle = handle;
}

void TileTreeModel::resetWithRoot() {
    beginResetModel();
    m_entries.clear();
    m_rootIndices.clear();
    m_tileToEntry.clear();

    if (m_handle) {
        char* raw = engine_get_tile_children_json(m_handle, 0);
        if (raw) {
            const auto doc = QJsonDocument::fromJson(QByteArray(raw));
            engine_free_string(raw);
            const auto arr = doc.array();
            for (int i = 0; i < arr.size(); ++i) {
                const QJsonObject obj = arr[i].toObject();
                NodeEntry e;
                e.tileId = static_cast<uint64_t>(obj.value("id").toInteger());
                e.label = obj.value("label").toString();
                e.visible = obj.value("visible").toBool(true);
                e.hasContent = obj.value("has_content").toBool(false);
                e.geometricError = obj.value("geometric_error").toDouble(0.0);
                e.mayHaveChildren = obj.value("child_count").toInt(0) > 0
                                    || obj.value("has_external_tileset").toBool(false);
                e.fetched = false;
                e.parentEntryIdx = -1;
                e.rowInParent = m_rootIndices.size();
                const int idx = m_entries.size();
                m_entries.push_back(e);
                m_rootIndices.push_back(idx);
                m_tileToEntry.insert(e.tileId, idx);
            }
        }
    }
    endResetModel();
}

void TileTreeModel::refreshExternalParent(uint64_t parentTileId) {
    auto it = m_tileToEntry.find(parentTileId);
    if (it == m_tileToEntry.end()) {
        return;
    }
    const int entryIdx = it.value();
    m_entries[entryIdx].mayHaveChildren = true;
    if (m_entries[entryIdx].fetched) {
        const QModelIndex parentModelIdx = createIndex(m_entries[entryIdx].rowInParent, 0, quintptr(entryIdx));
        appendChildrenFromEngine(entryIdx, parentTileId);
        emit dataChanged(parentModelIdx, parentModelIdx);
    }
}

void TileTreeModel::updateTileVisible(uint64_t tileId, bool visible) {
    auto it = m_tileToEntry.find(tileId);
    if (it == m_tileToEntry.end()) {
        return;
    }
    const int entryIdx = it.value();
    m_entries[entryIdx].visible = visible;
    const QModelIndex idx = createIndex(m_entries[entryIdx].rowInParent, 0, quintptr(entryIdx));
    emit dataChanged(idx, idx, {VisibleRole});
}

QModelIndex TileTreeModel::indexForTile(uint64_t tileId) const {
    auto it = m_tileToEntry.find(tileId);
    if (it == m_tileToEntry.end()) {
        return QModelIndex();
    }
    const int entryIdx = it.value();
    return createIndex(m_entries[entryIdx].rowInParent, 0, quintptr(entryIdx));
}

int TileTreeModel::rowCount(const QModelIndex& parent) const {
    if (!parent.isValid()) {
        return m_rootIndices.size();
    }
    const int idx = static_cast<int>(parent.internalId());
    if (idx < 0 || idx >= m_entries.size()) {
        return 0;
    }
    return m_entries[idx].children.size();
}

int TileTreeModel::columnCount(const QModelIndex&) const {
    return 1;
}

QModelIndex TileTreeModel::index(int row, int column, const QModelIndex& parent) const {
    if (column != 0 || row < 0) {
        return QModelIndex();
    }
    if (!parent.isValid()) {
        if (row >= m_rootIndices.size()) {
            return QModelIndex();
        }
        return createIndex(row, 0, quintptr(m_rootIndices[row]));
    }
    const int pIdx = static_cast<int>(parent.internalId());
    if (pIdx < 0 || pIdx >= m_entries.size()) {
        return QModelIndex();
    }
    const auto& children = m_entries[pIdx].children;
    if (row >= children.size()) {
        return QModelIndex();
    }
    return createIndex(row, 0, quintptr(children[row]));
}

QModelIndex TileTreeModel::parent(const QModelIndex& child) const {
    if (!child.isValid()) {
        return QModelIndex();
    }
    const int idx = static_cast<int>(child.internalId());
    if (idx < 0 || idx >= m_entries.size()) {
        return QModelIndex();
    }
    const int pIdx = m_entries[idx].parentEntryIdx;
    if (pIdx < 0 || pIdx >= m_entries.size()) {
        return QModelIndex();
    }
    return createIndex(m_entries[pIdx].rowInParent, 0, quintptr(pIdx));
}

QVariant TileTreeModel::data(const QModelIndex& index, int role) const {
    if (!index.isValid()) {
        return {};
    }
    const int idx = static_cast<int>(index.internalId());
    if (idx < 0 || idx >= m_entries.size()) {
        return {};
    }
    const auto& e = m_entries[idx];
    switch (role) {
    case Qt::DisplayRole:
    case LabelRole:
        return e.label;
    case TileIdRole:
        return static_cast<qulonglong>(e.tileId);
    case VisibleRole:
        return e.visible;
    case HasContentRole:
        return e.hasContent;
    case GeometricErrorRole:
        return e.geometricError;
    default:
        return {};
    }
}

bool TileTreeModel::hasChildren(const QModelIndex& parent) const {
    if (!parent.isValid()) {
        return !m_rootIndices.isEmpty();
    }
    const int idx = static_cast<int>(parent.internalId());
    if (idx < 0 || idx >= m_entries.size()) {
        return false;
    }
    const auto& e = m_entries[idx];
    return e.fetched ? !e.children.isEmpty() : e.mayHaveChildren;
}

bool TileTreeModel::canFetchMore(const QModelIndex& parent) const {
    if (!parent.isValid() || !m_handle) {
        return false;
    }
    const int idx = static_cast<int>(parent.internalId());
    if (idx < 0 || idx >= m_entries.size()) {
        return false;
    }
    const auto& e = m_entries[idx];
    return e.mayHaveChildren && !e.fetched;
}

void TileTreeModel::fetchMore(const QModelIndex& parent) {
    if (!parent.isValid() || !m_handle) {
        return;
    }
    const int idx = static_cast<int>(parent.internalId());
    if (idx < 0 || idx >= m_entries.size()) {
        return;
    }
    if (m_entries[idx].fetched) {
        return;
    }
    m_entries[idx].fetched = true;
    appendChildrenFromEngine(idx, m_entries[idx].tileId);
}

void TileTreeModel::appendChildrenFromEngine(int parentEntryIdx, uint64_t parentTileId) {
    if (!m_handle) {
        return;
    }
    char* raw = engine_get_tile_children_json(m_handle, parentTileId);
    if (!raw) {
        return;
    }
    const auto doc = QJsonDocument::fromJson(QByteArray(raw));
    engine_free_string(raw);
    const auto arr = doc.array();

    QVector<NodeEntry> pending;
    for (int i = 0; i < arr.size(); ++i) {
        const QJsonObject obj = arr[i].toObject();
        const uint64_t cid = static_cast<uint64_t>(obj.value("id").toInteger());
        if (m_tileToEntry.contains(cid)) {
            continue;
        }
        NodeEntry e;
        e.tileId = cid;
        e.label = obj.value("label").toString();
        e.visible = obj.value("visible").toBool(true);
        e.hasContent = obj.value("has_content").toBool(false);
        e.geometricError = obj.value("geometric_error").toDouble(0.0);
        e.mayHaveChildren = obj.value("child_count").toInt(0) > 0
                            || obj.value("has_external_tileset").toBool(false);
        e.fetched = false;
        e.parentEntryIdx = parentEntryIdx;
        pending.push_back(e);
    }
    if (pending.isEmpty()) {
        return;
    }

    const int firstRow = m_entries[parentEntryIdx].children.size();
    const int lastRow = firstRow + pending.size() - 1;
    const QModelIndex parentModelIdx =
        createIndex(m_entries[parentEntryIdx].rowInParent, 0, quintptr(parentEntryIdx));
    beginInsertRows(parentModelIdx, firstRow, lastRow);
    for (int i = 0; i < pending.size(); ++i) {
        NodeEntry item = pending[i];
        item.rowInParent = firstRow + i;
        const int newIdx = m_entries.size();
        m_entries.push_back(item);
        m_entries[parentEntryIdx].children.push_back(newIdx);
        m_tileToEntry.insert(item.tileId, newIdx);
    }
    endInsertRows();
}

QHash<int, QByteArray> TileTreeModel::roleNames() const {
    return {
        {TileIdRole, "tileId"},
        {LabelRole, "label"},
        {VisibleRole, "tileVisible"},
        {HasContentRole, "hasContent"},
        {GeometricErrorRole, "geometricError"},
    };
}
