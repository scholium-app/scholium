//! 语义图 → 可绘制图元的最小布局器。
//!
//! 为什么必须存在：把正文投影成纯文本会让"数学结构"这一项验收退化成模型层测试，
//! 候选之间也无法比较——分数得真的画出分数线，上下标得真的错位，矩阵得真的成网格。
//!
//! # 坐标系与度量约定
//!
//! - 所有坐标相对所属布局的左上角，单位是逻辑像素。
//! - [`Item::Text`] 给出的是**基线**纵坐标，不是绘制框上沿。渲染方必须用
//!   [`Item::top_of`] 换算出上沿，不能各自猜字体度量——早期版本就是因为各方各自按
//!   "文本框上沿"摆放，上下标相对基线偏了一整行。
//! - [`ASCENT_RATIO`] 是"基线到绘制框上沿"相对字号的近似比例。这是**近似值**
//!   （真值取决于字体 ascent），但所有图元共用同一个近似：同字号文本一定精确对齐，
//!   不同字号之间只有与字号差成正比的小误差。
//! - 上下标偏移用排版惯例（上标 −0.42em、下标 +0.20em），不再用临时常数。
//!
//! 这是刻意简化的布局器：不做字距调整、换行、双向文本或真字体度量。
//! 它够用来比较"候选能否渲染结构"，不足以评估排版质量。

use crate::doc::{Document, NodeKind};
use crate::ids::NodeId;

/// 基线到绘制框上沿相对字号的比例（近似值，见模块说明）。
pub const ASCENT_RATIO: f32 = 0.88;

/// 上标基线相对底基线的偏移（em，负值表示抬高）。
const SUPERSCRIPT_SHIFT: f32 = -0.42;

/// 下标基线相对底基线的偏移（em）。
const SUBSCRIPT_SHIFT: f32 = 0.20;

/// 文本图元对应的源位置，用于把光标与点击映射回语义图。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceSpan {
    /// 文本叶子节点。
    pub node: NodeId,
    /// 该图元内容在叶子里的起始字节偏移（UTF-8）。
    pub start_byte: usize,
}

/// 文本光标的几何位置，相对所属布局的原点。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caret {
    /// 光标左边界。
    pub x: f32,
    /// 光标所在行的基线。
    pub baseline: f32,
    /// 该处文本的字号，光标高度由它推出。
    pub size: f32,
}

/// 一个可绘制图元。坐标相对所属布局的原点，单位是逻辑像素。
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// 一段文本。`(x, baseline)` 是**基线左端**。
    Text {
        /// 左边界。
        x: f32,
        /// 基线纵坐标。
        baseline: f32,
        /// 字号。
        size: f32,
        /// 内容。
        content: String,
        /// 源位置。合成文本（括号、根号等）为 `None`。
        source: Option<SourceSpan>,
    },
    /// 一条实心线，用于分数线与根号上横线。
    Rule {
        /// 左边界。
        x: f32,
        /// 上边界。
        y: f32,
        /// 宽度。
        width: f32,
        /// 厚度。
        height: f32,
    },
}

impl Item {
    /// 把基线纵坐标换算成绘制框上沿。渲染方必须用它，不要自己猜度量。
    pub fn top_of(baseline: f32, size: f32) -> f32 {
        baseline - ASCENT_RATIO * size
    }
}

/// 布局度量。全部是近似值，见模块说明。
#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    /// 正文字号。
    pub font_size: f32,
    /// 分数线与分子/分母的间距。
    pub line_gap: f32,
    /// 上下标相对正文的缩放。
    pub script_scale: f32,
    /// 矩阵单元格内边距。
    pub cell_padding: f32,
    /// 块之间的间距。
    pub block_gap: f32,
    /// 线宽。
    pub rule_thickness: f32,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            font_size: 20.0,
            line_gap: 4.0,
            script_scale: 0.7,
            cell_padding: 8.0,
            block_gap: 8.0,
            rule_thickness: 1.4,
        }
    }
}

impl Metrics {
    /// 按比例缩放度量，用于上下标等嵌套场景。
    fn scaled(self, factor: f32) -> Self {
        Self {
            font_size: self.font_size * factor,
            line_gap: self.line_gap * factor,
            cell_padding: self.cell_padding * factor,
            block_gap: self.block_gap * factor,
            rule_thickness: (self.rule_thickness * factor).max(0.8),
            ..self
        }
    }
}

/// 布局结果。
#[derive(Clone, Debug, Default)]
pub struct Layout {
    /// 图元，坐标相对本布局原点。
    pub items: Vec<Item>,
    /// 外框宽度。
    pub width: f32,
    /// 外框高度。
    pub height: f32,
    /// 基线相对本布局顶部的距离，用于行内对齐。
    pub baseline: f32,
}

impl Layout {
    /// 在布局里定位文本光标。
    ///
    /// 返回相对**本布局**原点的几何；调用方负责再加上布局在页面上的原点。
    /// 找不到对应节点时返回 `None`（例如光标停在结构槽位上）。
    pub fn caret(&self, node: NodeId, byte_offset: usize) -> Option<Caret> {
        for item in &self.items {
            let Item::Text {
                x,
                baseline,
                size,
                content,
                source: Some(span),
            } = item
            else {
                continue;
            };
            if span.node != node {
                continue;
            }
            let local = byte_offset.saturating_sub(span.start_byte);
            // 字节偏移可能落在字素中间：取不到就退回到整段宽度，绝不按字节切字符串。
            let prefix = content.get(..local).unwrap_or(content.as_str());
            return Some(Caret {
                x: x + text_width(prefix, *size),
                baseline: *baseline,
                size: *size,
            });
        }
        None
    }
}

/// 布局整个文档。
pub fn layout_document(doc: &Document) -> Layout {
    layout_node(doc, doc.root(), Metrics::default())
}

/// 布局单个节点。
pub fn layout_node(doc: &Document, node: NodeId, metrics: Metrics) -> Layout {
    let Ok(current) = doc.node(node) else {
        return Layout::default();
    };

    match current.kind {
        NodeKind::Text | NodeKind::Raw => {
            let content = doc.text_of(node).unwrap_or_default();
            text_layout_at(
                &content,
                metrics,
                Some(SourceSpan {
                    node,
                    start_byte: 0,
                }),
            )
        }
        NodeKind::Document => block(slot_layouts(doc, node, 0, metrics), metrics.block_gap),
        NodeKind::Paragraph => inline(slot_layouts(doc, node, 0, metrics), 0.0),
        NodeKind::Heading => inline(
            slot_layouts(doc, node, 0, metrics.scaled(1.3)),
            metrics.block_gap,
        ),
        NodeKind::Math => inline(slot_layouts(doc, node, 0, metrics), 0.0),
        NodeKind::Fraction => fraction(doc, node, metrics),
        NodeKind::Sqrt => sqrt(doc, node, metrics),
        NodeKind::Script => script(doc, node, metrics),
        NodeKind::Delimited => delimited(doc, node, metrics),
        NodeKind::Matrix => matrix(doc, node, metrics),
    }
}

/// 文本的近似宽度：CJK 按 1em，其余按 0.6em。
fn text_width(content: &str, size: f32) -> f32 {
    content
        .chars()
        .map(|ch| {
            if (ch as u32) >= 0x2E80 {
                size
            } else {
                size * 0.6
            }
        })
        .sum()
}

/// 单段文本：基线在 `ASCENT_RATIO * size` 处。用于合成文本（括号、根号等）。
fn text_layout(content: &str, metrics: Metrics) -> Layout {
    text_layout_at(content, metrics, None)
}

/// 单段文本，并记录它来自哪个节点的哪个字节区间。
fn text_layout_at(content: &str, metrics: Metrics, source: Option<SourceSpan>) -> Layout {
    let size = metrics.font_size;
    if content.is_empty() {
        return Layout::default();
    }
    let baseline = ASCENT_RATIO * size;
    Layout {
        items: vec![Item::Text {
            x: 0.0,
            baseline,
            size,
            content: content.to_string(),
            source,
        }],
        width: text_width(content, size),
        height: size * 1.2,
        baseline,
    }
}

fn shift(items: &mut [Item], dx: f32, dy: f32) {
    for item in items.iter_mut() {
        match item {
            Item::Text { x, baseline, .. } => {
                *x += dx;
                *baseline += dy;
            }
            Item::Rule { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
        }
    }
}

/// 把整块布局整体下移，保证没有负坐标。
fn drop_to_origin(layout: &mut Layout) {
    let lowest = layout
        .items
        .iter()
        .map(|item| match item {
            Item::Text {
                baseline, size, ..
            } => Item::top_of(*baseline, *size),
            Item::Rule { y, .. } => *y,
        })
        .fold(f32::MAX, f32::min);
    if lowest < 0.0 {
        let dy = -lowest;
        shift(&mut layout.items, 0.0, dy);
        layout.baseline += dy;
    }
}

/// 水平排列，按基线对齐。
fn inline(parts: Vec<Layout>, gap: f32) -> Layout {
    let parts: Vec<Layout> = parts.into_iter().filter(|p| p.width > 0.0).collect();
    if parts.is_empty() {
        return Layout::default();
    }
    let baseline = parts.iter().map(|p| p.baseline).fold(f32::MIN, f32::max);
    let mut items = Vec::new();
    let mut x = 0.0_f32;
    let mut height = 0.0_f32;
    for mut part in parts {
        let dy = baseline - part.baseline;
        shift(&mut part.items, x, dy);
        items.extend(part.items);
        x += part.width + gap;
        height = height.max(dy + part.height);
    }
    Layout {
        items,
        width: (x - gap).max(0.0),
        height,
        baseline,
    }
}

/// 垂直排列。
fn block(parts: Vec<Layout>, gap: f32) -> Layout {
    if parts.is_empty() {
        return Layout::default();
    }
    let baseline = parts.first().map(|p| p.baseline).unwrap_or(0.0);
    let mut items = Vec::new();
    let mut y = 0.0_f32;
    let mut width = 0.0_f32;
    for mut part in parts {
        shift(&mut part.items, 0.0, y);
        items.extend(part.items);
        width = width.max(part.width);
        y += part.height + gap;
    }
    Layout {
        items,
        width,
        height: (y - gap).max(0.0),
        baseline,
    }
}

fn slot_layouts(doc: &Document, node: NodeId, slot: usize, metrics: Metrics) -> Vec<Layout> {
    doc.slot(node, slot)
        .unwrap_or(&[])
        .iter()
        .map(|child| layout_node(doc, *child, metrics))
        .collect()
}

/// 分数：分子在上、分数线居中、分母在下。
///
/// 行内基线取**数学轴**（分数线中心）下移 0.25em 的位置，使分数与相邻文本对齐。
fn fraction(doc: &Document, node: NodeId, metrics: Metrics) -> Layout {
    let numerator = slot_layouts(doc, node, 0, metrics)
        .into_iter()
        .next()
        .unwrap_or_default();
    let denominator = slot_layouts(doc, node, 1, metrics)
        .into_iter()
        .next()
        .unwrap_or_default();

    let width = numerator
        .width
        .max(denominator.width)
        .max(metrics.font_size * 0.8);
    let bar_y = numerator.height + metrics.line_gap;
    let denominator_y = bar_y + metrics.rule_thickness + metrics.line_gap;

    let mut items = Vec::new();
    let mut top = numerator;
    shift(&mut top.items, (width - top.width) / 2.0, 0.0);
    items.extend(top.items);

    items.push(Item::Rule {
        x: 0.0,
        y: bar_y,
        width,
        height: metrics.rule_thickness,
    });

    let mut bottom = denominator;
    shift(
        &mut bottom.items,
        (width - bottom.width) / 2.0,
        denominator_y,
    );
    let height = denominator_y + bottom.height;
    items.extend(bottom.items);

    Layout {
        items,
        width,
        height,
        baseline: bar_y + metrics.rule_thickness / 2.0 + metrics.font_size * 0.25,
    }
}

/// 根式：上横线 + 根号 + 被开方数，三者基线一致。
fn sqrt(doc: &Document, node: NodeId, metrics: Metrics) -> Layout {
    let radicand = slot_layouts(doc, node, 0, metrics)
        .into_iter()
        .next()
        .unwrap_or_default();
    let sign = text_layout("\u{221A}", metrics);
    let sign_width = sign.width;
    let overlay = metrics.rule_thickness + metrics.line_gap * 0.5;

    let mut body = radicand;
    shift(&mut body.items, sign_width, overlay);
    let body_baseline = body.baseline + overlay;
    let body_height = body.height + overlay;

    let mut sign_items = sign.items;
    shift(&mut sign_items, 0.0, overlay);

    let mut items = vec![Item::Rule {
        x: sign_width,
        y: 0.0,
        width: body.width,
        height: metrics.rule_thickness,
    }];
    items.extend(sign_items);
    items.extend(body.items);

    Layout {
        items,
        width: sign_width + body.width,
        height: body_height,
        baseline: body_baseline,
    }
}

/// 上下标：底、下、上三个槽位。
///
/// 上标基线抬 `0.42em`、下标基线降 `0.20em`（相对底的字号），这是排版惯例值，
/// 不再是"看起来差不多"的临时常数。
fn script(doc: &Document, node: NodeId, metrics: Metrics) -> Layout {
    let base = slot_layouts(doc, node, 0, metrics)
        .into_iter()
        .next()
        .unwrap_or_default();
    let small = metrics.scaled(metrics.script_scale);
    let subscript = slot_layouts(doc, node, 1, small)
        .into_iter()
        .next()
        .unwrap_or_default();
    let superscript = slot_layouts(doc, node, 2, small)
        .into_iter()
        .next()
        .unwrap_or_default();

    let base_width = base.width;
    let base_height = base.height;
    let base_baseline = base.baseline;
    let sub_width = subscript.width;
    let sup_width = superscript.width;
    let script_x = base_width;

    let superscript_baseline = base_baseline + SUPERSCRIPT_SHIFT * metrics.font_size;
    let subscript_baseline = base_baseline + SUBSCRIPT_SHIFT * metrics.font_size;

    let mut items = base.items;

    let sup_height = if sup_width > 0.0 {
        let mut sup = superscript;
        let dy = superscript_baseline - sup.baseline;
        shift(&mut sup.items, script_x, dy);
        let height = dy + sup.height;
        items.extend(sup.items);
        height.max(0.0)
    } else {
        0.0
    };

    let sub_height = if sub_width > 0.0 {
        let mut sub = subscript;
        let dy = subscript_baseline - sub.baseline;
        shift(&mut sub.items, script_x, dy);
        let height = dy + sub.height;
        items.extend(sub.items);
        height.max(0.0)
    } else {
        0.0
    };

    let mut layout = Layout {
        items,
        width: base_width + sub_width.max(sup_width),
        height: base_height.max(sup_height).max(sub_height),
        baseline: base_baseline,
    };
    drop_to_origin(&mut layout);
    layout
}

/// 定界符：左右括号包裹内容。
fn delimited(doc: &Document, node: NodeId, metrics: Metrics) -> Layout {
    let open = text_layout("(", metrics);
    let body = slot_layouts(doc, node, 0, metrics)
        .into_iter()
        .next()
        .unwrap_or_default();
    let close = text_layout(")", metrics);
    inline(vec![open, body, close], metrics.font_size * 0.15)
}

/// 矩阵：单元格按固定列数排成网格。
///
/// 简化：列数固定为 2（只有一个单元格时为 1 列），不读取文档里的行列声明。
fn matrix(doc: &Document, node: NodeId, metrics: Metrics) -> Layout {
    let cells = slot_layouts(doc, node, 0, metrics);
    if cells.is_empty() {
        return Layout::default();
    }
    let columns = if cells.len() > 1 { 2 } else { 1 };
    let row_count = cells.len().div_ceil(columns);
    let mut column_widths = vec![0.0_f32; columns];
    let mut row_heights = vec![0.0_f32; row_count];
    for (index, cell) in cells.iter().enumerate() {
        let column = index % columns;
        let row = index / columns;
        column_widths[column] = column_widths[column].max(cell.width);
        row_heights[row] = row_heights[row].max(cell.height);
    }

    let pad = metrics.cell_padding;
    let mut items = Vec::new();
    let mut y = 0.0_f32;
    for row in 0..row_count {
        let mut x = 0.0_f32;
        for column in 0..columns {
            let index = row * columns + column;
            if let Some(cell) = cells.get(index) {
                let mut placed = cell.clone();
                shift(
                    &mut placed.items,
                    x + (column_widths[column] - placed.width) / 2.0,
                    y + (row_heights[row] - placed.height) / 2.0,
                );
                items.extend(placed.items);
            }
            x += column_widths[column] + pad;
        }
        y += row_heights[row] + pad;
    }

    let width = column_widths.iter().sum::<f32>() + pad * (columns.saturating_sub(1) as f32);
    let height = row_heights.iter().sum::<f32>() + pad * (row_count.saturating_sub(1) as f32);
    Layout {
        items,
        width,
        height,
        // 矩阵按垂直中心对齐正文轴线。
        baseline: height / 2.0,
    }
}
