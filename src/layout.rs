// SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
// SPDX-License-Identifier: Apache-2.0

use crate::config::{Axis, Demo, Layout as Kind, LayoutNode, LayoutPreset, PaneConfig};
use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct Geometry {
    /// Always indexed by pane configuration order, regardless of tree order.
    pub panes: Vec<Rect>,
    pub dividers: Vec<Divider>,
}
#[derive(Clone, Debug)]
pub struct Divider {
    axis: Axis,
    area: Rect,
    boundary: u16,
    path: Vec<usize>,
    index: usize,
    preset_pair: Option<(usize, usize)>,
}
impl Divider {
    pub fn contains(&self, x: u16, y: u16) -> bool {
        let position = match self.axis {
            Axis::Columns => x,
            Axis::Rows => y,
        };
        self.area.contains((x, y).into())
            && (position == self.boundary || position.checked_add(1) == Some(self.boundary))
    }
}
fn split(area: Rect, axis: Axis, weights: impl Iterator<Item = u16>) -> Vec<Rect> {
    let weights: Vec<_> = weights.collect();
    let total: u32 = weights.iter().map(|&w| u32::from(w)).sum();
    Layout::default()
        .direction(match axis {
            Axis::Columns => Direction::Horizontal,
            Axis::Rows => Direction::Vertical,
        })
        .constraints(
            weights
                .iter()
                .map(|&w| Constraint::Ratio(u32::from(w), total.max(1))),
        )
        .split(area)
        .to_vec()
}
fn divider(
    a: Rect,
    b: Rect,
    axis: Axis,
    path: Vec<usize>,
    index: usize,
    preset_pair: Option<(usize, usize)>,
) -> Divider {
    Divider {
        axis,
        area: a.union(b),
        boundary: match axis {
            Axis::Columns => b.x,
            Axis::Rows => b.y,
        },
        path,
        index,
        preset_pair,
    }
}
pub fn panes(area: Rect, kind: &Kind, configs: &[PaneConfig]) -> Geometry {
    let mut geometry = Geometry {
        panes: vec![Rect::default(); configs.len()],
        dividers: vec![],
    };
    match kind {
        Kind::Tree(root) => place(root, area, configs, &mut geometry, &mut vec![]),
        Kind::Preset(preset) => {
            let (axis, cols, bands) = match preset {
                LayoutPreset::Columns => (Axis::Columns, configs.len(), vec![area]),
                LayoutPreset::Rows => (Axis::Rows, configs.len(), vec![area]),
                LayoutPreset::Grid => {
                    let cols = (configs.len() as f64).sqrt().ceil() as usize;
                    let rows = configs.len().div_ceil(cols.max(1));
                    (
                        Axis::Columns,
                        cols,
                        split(area, Axis::Rows, std::iter::repeat_n(1, rows)),
                    )
                }
            };
            for (row, (group, band)) in configs.chunks(cols.max(1)).zip(bands).enumerate() {
                let rects = split(band, axis, group.iter().map(|p| p.weight));
                let start = row * cols;
                geometry.panes[start..start + group.len()].copy_from_slice(&rects);
                for (i, pair) in rects.windows(2).enumerate() {
                    geometry.dividers.push(divider(
                        pair[0],
                        pair[1],
                        axis,
                        vec![],
                        i,
                        Some((start + i, start + i + 1)),
                    ));
                }
            }
        }
    }
    geometry
}
fn place(
    node: &LayoutNode,
    area: Rect,
    configs: &[PaneConfig],
    geometry: &mut Geometry,
    path: &mut Vec<usize>,
) {
    match node {
        LayoutNode::Pane { pane, .. } => {
            let index = configs
                .iter()
                .position(|p| p.id == *pane)
                .expect("validated layout pane");
            geometry.panes[index] = area;
        }
        LayoutNode::Split {
            direction,
            children,
            ..
        } => {
            let rects = split(area, *direction, children.iter().map(LayoutNode::weight));
            // Parents come first; hit-testing in reverse prefers the deepest split.
            for (i, pair) in rects.windows(2).enumerate() {
                geometry.dividers.push(divider(
                    pair[0],
                    pair[1],
                    *direction,
                    path.clone(),
                    i,
                    None,
                ));
            }
            for (i, (child, rect)) in children.iter().zip(rects).enumerate() {
                path.push(i);
                place(child, rect, configs, geometry, path);
                path.pop();
            }
        }
    }
}
/// Resize adjacent siblings, whether panes or whole nested groups.
pub fn resize_pair(demo: &mut Demo, cue: Option<usize>, divider: &Divider, x: u16, y: u16) {
    let (start, end, position) = match divider.axis {
        Axis::Columns => (divider.area.x, divider.area.right(), x),
        Axis::Rows => (divider.area.y, divider.area.bottom(), y),
    };
    if end.saturating_sub(start) < 8 {
        return;
    }
    let position = position.clamp(start + 4, end - 4);
    let mut weights: Vec<&mut u16>;
    let (first, second) = if let Some((a, b)) = divider.preset_pair {
        weights = demo.panes.iter_mut().map(|p| &mut p.weight).collect();
        (a, b)
    } else {
        let Kind::Tree(mut_node) = demo.layout_for_mut(cue) else {
            return;
        };
        let mut node = mut_node;
        for &i in &divider.path {
            let LayoutNode::Split { children, .. } = node else {
                return;
            };
            let Some(child) = children.get_mut(i) else {
                return;
            };
            node = child;
        }
        let LayoutNode::Split { children, .. } = node else {
            return;
        };
        weights = children.iter_mut().map(LayoutNode::weight_mut).collect();
        (divider.index, divider.index + 1)
    };
    if second >= weights.len() {
        return;
    }
    // Normalize the sibling group to retain useful precision and bounded weights.
    let max = weights.iter().map(|w| **w).max().unwrap_or(1) as f64;
    for weight in &mut weights {
        **weight = ((**weight as f64 / max) * 400.0).round().max(1.0) as u16;
    }
    let total = *weights[first] + *weights[second];
    let size = ((position - start) as f64 / (end - start) as f64 * total as f64).round() as u16;
    *weights[first] = size.clamp(1, total - 1);
    *weights[second] = total - *weights[first];
}

/// Use the nearest split on the requested axis that contains the focused pane.
pub fn resize_focused(
    demo: &mut Demo,
    cue: Option<usize>,
    dividers: &[Divider],
    pane: Rect,
    axis: Axis,
    delta: i16,
) -> bool {
    let Some(edge) = dividers.iter().rev().find(|edge| {
        edge.axis == axis
            && edge.area.intersection(pane) == pane
            && pane.width > 0
            && pane.height > 0
    }) else {
        return false;
    };
    let first = match axis {
        Axis::Columns => pane.right() <= edge.boundary,
        Axis::Rows => pane.bottom() <= edge.boundary,
    };
    let position = edge
        .boundary
        .saturating_add_signed(if first { delta } else { -delta });
    let before = (
        demo.layout_for(cue).clone(),
        demo.panes.iter().map(|p| p.weight).collect::<Vec<_>>(),
    );
    resize_pair(demo, cue, edge, position, position);
    before
        != (
            demo.layout_for(cue).clone(),
            demo.panes.iter().map(|p| p.weight).collect::<Vec<_>>(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_geometry_resizes_groups_and_maps_ids() {
        let mut demo = Demo::parse(include_str!("../examples/nested-layout.toml")).unwrap();
        demo.panes.reverse();
        let area = Rect::new(10, 4, 120, 60);
        let geometry = panes(area, &demo.layout, &demo.panes);
        let rect = |id: &str| geometry.panes[demo.panes.iter().position(|p| p.id == id).unwrap()];
        assert_eq!(rect("presenter"), Rect::new(10, 4, 80, 30));
        assert_eq!(rect("build"), Rect::new(10, 34, 40, 30));
        assert_eq!(rect("tests"), Rect::new(50, 34, 40, 30));
        assert_eq!(rect("logs"), Rect::new(90, 4, 40, 30));
        assert_eq!(rect("notes"), Rect::new(90, 34, 40, 30));
        assert_eq!(geometry.dividers.len(), 4);
        // Resize the outer column: both nested rows change width together.
        let edge = geometry
            .dividers
            .iter()
            .rev()
            .find(|d| d.contains(89, 12))
            .unwrap();
        resize_pair(&mut demo, None, edge, 70, 12);
        let resized = panes(area, &demo.layout, &demo.panes);
        let by_id =
            |g: &Geometry, id: &str| g.panes[demo.panes.iter().position(|p| p.id == id).unwrap()];
        assert_eq!(by_id(&resized, "presenter").width, 60);
        assert_eq!(by_id(&resized, "logs").x, 70);
        assert_eq!(by_id(&resized, "notes").x, 70);
        // Resize just the lower-left inner columns; right-hand panes stay put.
        let edge = resized
            .dividers
            .iter()
            .rev()
            .find(|d| d.contains(39, 45))
            .unwrap();
        let before = resized.panes.clone();
        resize_pair(&mut demo, None, edge, 50, 45);
        let after = panes(area, &demo.layout, &demo.panes);
        for id in ["presenter", "logs", "notes"] {
            let i = demo.panes.iter().position(|p| p.id == id).unwrap();
            assert_eq!(before[i], after.panes[i]);
        }
        demo.validate().unwrap();
        for (w, h) in [(1, 1), (8, 4), (0, 0)] {
            let tiny = Rect::new(0, 0, w, h);
            let geometry = panes(tiny, &demo.layout, &demo.panes);
            for r in geometry.panes {
                assert_eq!(r.intersection(tiny), r);
            }
        }
    }
    #[test]
    fn grid_and_resize_preserve_panes() {
        let mut demo = Demo {
            panes: vec![PaneConfig::default(); 3],
            layout: Kind::Preset(LayoutPreset::Grid),
            ..Default::default()
        };
        let geometry = panes(Rect::new(0, 0, 100, 30), &demo.layout, &demo.panes);
        assert_eq!(geometry.panes.len(), 3);
        assert_eq!(geometry.panes[0].y, geometry.panes[1].y);
        assert!(geometry.panes[2].y > geometry.panes[0].y);
        resize_pair(&mut demo, None, &geometry.dividers[0], 70, 2);
        assert!(demo.panes[0].weight > demo.panes[1].weight);
    }
}
