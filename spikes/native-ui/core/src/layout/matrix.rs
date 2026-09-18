//! Matrix composition, preserving translated semantic boxes.
use super::*;

/// 矩阵：单元格按固定列数排成网格。
///
/// 简化：列数固定为 2（只有一个单元格时为 1 列），不读取文档里的行列声明。
pub(super) fn matrix(doc: &Document, node: NodeId, metrics: Metrics) -> Layout {
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
    let mut bounds = Vec::new();
    let mut y = 0.0_f32;
    for row in 0..row_count {
        let mut x = 0.0_f32;
        for column in 0..columns {
            let index = row * columns + column;
            if let Some(cell) = cells.get(index) {
                let mut placed = cell.clone();
                let dx = x + (column_widths[column] - placed.width) / 2.0;
                let dy = y + (row_heights[row] - placed.height) / 2.0;
                shift(&mut placed, dx, dy);
                bounds.extend(placed.bounds);
                items.extend(placed.items);
            }
            x += column_widths[column] + pad;
        }
        y += row_heights[row] + pad;
    }

    let width = column_widths.iter().sum::<f32>() + pad * (columns.saturating_sub(1) as f32);
    let height = row_heights.iter().sum::<f32>() + pad * (row_count.saturating_sub(1) as f32);
    Layout {
        bounds,
        items,
        width,
        height,
        // 矩阵按垂直中心对齐正文轴线。
        baseline: height / 2.0,
    }
}
