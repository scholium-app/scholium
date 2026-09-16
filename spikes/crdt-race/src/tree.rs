//! 结构树 CRDT：稳定节点标识 + LWW 寄存器。
//!
//! 树不是把 CRDT 序列套在树上，而是**每个字段一个 LWW 寄存器**：
//!
//! - `kind`：创建时写入一次，之后不变（节点标识唯一，所以没有并发创建同一节点的情况）。
//! - `parent` / `pos`：一个 "放置" 寄存器，包裹 / 解除包裹 / 移动都是写它。
//!   兄弟顺序用与文本同一套位置标识，因此 "把子节点搬到新父级" 与 "在兄弟间插一个" 是同一机制。
//! - `alive`：存活寄存器，删除与撤销删除都是写它。
//! - `attrs`：每个键一个 LWW 寄存器，值是 `Option<String>`（`None` 表示移除该属性）。
//!
//! 全部寄存器取 `(counter, actor)` 最大者，所以状态是操作集合的纯函数，与投递顺序无关。
//! 节点标识不随父级变化而改变——这正是 "远端在旧父级上输入的文本，在本地把该父级包裹之后
//! 依然落在同一节点" 的原因（`docs/DATA_MODEL.md` 的稳定身份要求）。
//!
//! 已知语义缺口：并发地互相移动两个节点可能让父指针成环。本 spike 只比较规范状态与渲染，
//! 渲染带深度上限，不会因此 panic；完整的树语义冲突策略不在本 spike 范围内。

use std::collections::{BTreeMap, HashMap};

use crate::codec::{Reader, Writer};
use crate::error::CrdtError;
use crate::ids::{Id, Lamport, NodeId};
use crate::model::NodeKind;
use crate::position::Position;

/// 一个属性键的寄存器。
#[derive(Clone, Debug)]
pub(crate) struct AttrReg {
    /// `None` 表示该属性已被移除。
    pub(crate) value: Option<String>,
    /// 最后写入时间戳。
    pub(crate) ts: Lamport,
}

/// 一个节点的全部寄存器。
#[derive(Clone, Debug)]
pub(crate) struct NodeRecord {
    /// 节点种类；创建操作到达前为 `None`。
    pub(crate) kind: Option<NodeKind>,
    /// 父节点。
    pub(crate) parent: NodeId,
    /// 兄弟间位置标识。
    pub(crate) pos: Position,
    /// 放置寄存器时间戳。
    pub(crate) place_ts: Lamport,
    /// 是否存活。
    pub(crate) alive: bool,
    /// 存活寄存器时间戳。
    pub(crate) alive_ts: Lamport,
    /// 属性寄存器。
    pub(crate) attrs: BTreeMap<String, AttrReg>,
}

impl NodeRecord {
    /// 尚未收到任何操作时的默认记录。任一操作先到达都会先建立它，因此默认值是确定性的。
    fn default_for() -> Self {
        Self {
            kind: None,
            parent: Id::root(),
            pos: vec![1],
            place_ts: Lamport::zero(),
            alive: true,
            alive_ts: Lamport::zero(),
            attrs: BTreeMap::new(),
        }
    }

    /// 读取属性值；属性不存在或已被移除时返回 `None`。
    pub(crate) fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).and_then(|reg| reg.value.as_deref())
    }
}

/// 结构树状态。
#[derive(Clone, Debug, Default)]
pub(crate) struct TreeCrdt {
    nodes: HashMap<NodeId, NodeRecord>,
}

impl TreeCrdt {
    /// 节点记录。
    pub(crate) fn node(&self, id: NodeId) -> Option<&NodeRecord> {
        self.nodes.get(&id)
    }

    /// 节点数量。
    pub(crate) fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 应用创建操作。重复或更旧的放置寄存器不覆盖新值。
    pub(crate) fn apply_create(
        &mut self,
        id: NodeId,
        kind: NodeKind,
        parent: NodeId,
        pos: Position,
        ts: Lamport,
    ) {
        let record = self.nodes.entry(id).or_insert_with(NodeRecord::default_for);
        if record.kind.is_none() {
            record.kind = Some(kind);
        }
        Self::place(record, parent, pos, ts);
    }

    /// 应用放置写入（包裹 / 解除包裹 / 移动）。
    pub(crate) fn apply_place(&mut self, id: NodeId, parent: NodeId, pos: Position, ts: Lamport) {
        let record = self.nodes.entry(id).or_insert_with(NodeRecord::default_for);
        Self::place(record, parent, pos, ts);
    }

    /// 应用存活位写入。
    pub(crate) fn apply_alive(&mut self, id: NodeId, alive: bool, ts: Lamport) {
        let record = self.nodes.entry(id).or_insert_with(NodeRecord::default_for);
        if ts > record.alive_ts {
            record.alive = alive;
            record.alive_ts = ts;
        }
    }

    /// 应用属性写入。
    pub(crate) fn apply_attr(&mut self, id: NodeId, key: &str, value: Option<&str>, ts: Lamport) {
        let record = self.nodes.entry(id).or_insert_with(NodeRecord::default_for);
        let register = record
            .attrs
            .entry(key.to_owned())
            .or_insert_with(|| AttrReg {
                value: None,
                ts: Lamport::zero(),
            });
        if ts > register.ts {
            register.value = value.map(str::to_owned);
            register.ts = ts;
        }
    }

    /// 存活子节点，按 `(位置标识, 节点标识)` 升序。
    pub(crate) fn children(&self, parent: NodeId) -> Vec<NodeId> {
        let mut items: Vec<(NodeId, &NodeRecord)> = self
            .nodes
            .iter()
            .filter(|(_, record)| record.alive && record.parent == parent)
            .map(|(id, record)| (*id, record))
            .collect();
        items.sort_by(|(left_id, left), (right_id, right)| {
            left.pos.cmp(&right.pos).then(left_id.cmp(right_id))
        });
        items.into_iter().map(|(id, _)| id).collect()
    }

    /// `id` 之后最近一个兄弟的位置标识，用于解除包裹时分配新位置的上界。
    pub(crate) fn next_sibling_pos(&self, id: NodeId) -> Option<&Position> {
        let me = self.nodes.get(&id)?;
        self.nodes
            .iter()
            .filter(|(other_id, other)| other.alive && other.parent == me.parent && **other_id != id)
            .filter(|(other_id, other)| {
                other.pos > me.pos || (other.pos == me.pos && **other_id > id)
            })
            .min_by(|(left_id, left), (right_id, right)| {
                left.pos.cmp(&right.pos).then(left_id.cmp(right_id))
            })
            .map(|(_, record)| &record.pos)
    }

    /// 规范状态：按节点标识升序写出全部寄存器。
    pub(crate) fn write_canonical(&self, w: &mut Writer) {
        let mut ids: Vec<&NodeId> = self.nodes.keys().collect();
        ids.sort();
        w.u32(ids.len() as u32);
        for id in ids {
            let record = self.nodes.get(id).expect("键来自同一 map");
            Self::write_record(w, *id, record);
        }
    }

    /// 完整快照（与规范状态同一布局）。
    pub(crate) fn write_snapshot(&self, w: &mut Writer) {
        self.write_canonical(w);
    }

    /// 从快照读回。
    ///
    /// # Errors
    ///
    /// 输入截断、判别式非法或位置标识不合法时返回错误。
    pub(crate) fn read_snapshot(r: &mut Reader<'_>) -> Result<Self, CrdtError> {
        let count = r.u32()? as usize;
        let mut nodes = HashMap::with_capacity(count);
        for _ in 0..count {
            let id = r.id()?;
            let kind_tag = r.u8()?;
            let kind = match kind_tag {
                0 => None,
                tag => Some(NodeKind::from_tag(tag).ok_or(CrdtError::InvalidTag {
                    what: "node kind",
                    tag,
                })?),
            };
            let parent = r.id()?;
            let pos = r.position()?;
            let place_ts = r.lamport()?;
            let alive = r.bool()?;
            let alive_ts = r.lamport()?;
            let attr_count = r.u32()? as usize;
            let mut attrs = BTreeMap::new();
            for _ in 0..attr_count {
                let key = r.str()?;
                let value = r.opt_str()?;
                let ts = r.lamport()?;
                attrs.insert(key, AttrReg { value, ts });
            }
            nodes.insert(
                id,
                NodeRecord {
                    kind,
                    parent,
                    pos,
                    place_ts,
                    alive,
                    alive_ts,
                    attrs,
                },
            );
        }
        Ok(Self { nodes })
    }

    fn place(record: &mut NodeRecord, parent: NodeId, pos: Position, ts: Lamport) {
        if ts > record.place_ts {
            record.parent = parent;
            record.pos = pos;
            record.place_ts = ts;
        }
    }

    fn write_record(w: &mut Writer, id: NodeId, record: &NodeRecord) {
        w.id(id);
        w.u8(record.kind.map_or(0, NodeKind::tag));
        w.id(record.parent);
        w.position(&record.pos);
        w.lamport(record.place_ts);
        w.bool(record.alive);
        w.lamport(record.alive_ts);
        w.u32(record.attrs.len() as u32);
        for (key, reg) in &record.attrs {
            w.str(key);
            w.opt_str(reg.value.as_deref());
            w.lamport(reg.ts);
        }
    }
}
