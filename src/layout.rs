// SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
// SPDX-License-Identifier: Apache-2.0

use crate::config::{Layout as Kind, PaneConfig};
use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn panes(area: Rect, kind: Kind, configs: &[PaneConfig]) -> Vec<Rect> {
    let split = |area, direction, panes: &[PaneConfig]| {
        let total: u32 = panes.iter().map(|p| p.weight as u32).sum();
        Layout::default()
            .direction(direction)
            .constraints(
                panes
                    .iter()
                    .map(|p| Constraint::Ratio(p.weight as u32, total.max(1))),
            )
            .split(area)
            .to_vec()
    };
    match kind {
        Kind::Columns => split(area, Direction::Horizontal, configs),
        Kind::Rows => split(area, Direction::Vertical, configs),
        Kind::Grid => {
            let cols = (configs.len() as f64).sqrt().ceil() as usize;
            let rows = configs.len().div_ceil(cols.max(1));
            let bands =
                Layout::vertical(vec![Constraint::Ratio(1, rows.max(1) as u32); rows]).split(area);
            configs
                .chunks(cols.max(1))
                .zip(bands.iter())
                .flat_map(|(p, &r)| split(r, Direction::Horizontal, p))
                .collect()
        }
    }
}

/// Drag a shared edge; grid row heights stay equal, columns resize within a row.
pub fn resize_pair(
    configs: &mut [PaneConfig],
    rects: &[Rect],
    kind: Kind,
    index: usize,
    x: u16,
    y: u16,
) {
    if index + 1 >= rects.len() {
        return;
    }
    let (a, b) = (rects[index], rects[index + 1]);
    let vertical = kind == Kind::Rows;
    if !vertical && a.y != b.y {
        return;
    }
    let (start, end, position) = if vertical {
        (a.y, b.bottom(), y)
    } else {
        (a.x, b.right(), x)
    };
    if end.saturating_sub(start) < 8 {
        return;
    }
    let position = position.clamp(start + 4, end - 4);
    // Normalize all weights first to keep adjacent dragging stable with defaults of 1.
    let max = configs.iter().map(|p| p.weight).max().unwrap_or(1) as f64;
    for config in configs.iter_mut() {
        config.weight = ((config.weight as f64 / max) * 400.0).round().max(1.0) as u16;
    }
    let total = configs[index].weight + configs[index + 1].weight;
    let first = ((position - start) as f64 / (end - start) as f64 * total as f64).round() as u16;
    configs[index].weight = first.clamp(1, total - 1);
    configs[index + 1].weight = total - configs[index].weight;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_and_resize_preserve_panes() {
        let mut configs = vec![PaneConfig::default(); 3];
        let rects = panes(Rect::new(0, 0, 100, 30), Kind::Grid, &configs);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].y, rects[1].y);
        assert!(rects[2].y > rects[0].y);
        resize_pair(&mut configs, &rects, Kind::Grid, 0, 70, 2);
        assert!(configs[0].weight > configs[1].weight);
    }
}
