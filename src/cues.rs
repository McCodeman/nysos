use crate::config::Queue;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Paragraph},
};

/// Browsing is independent of the execution cursor: arrows never run commands.
#[derive(Default)]
pub struct CueList {
    pub focused: bool,
    pub selected: usize,
    pub area: Rect,
    offset: usize,
}
impl CueList {
    pub fn select(&mut self, index: usize, count: usize) {
        self.selected = index.min(count.saturating_sub(1));
        let height = self.area.height.saturating_sub(2).max(1) as usize;
        self.offset = self.offset.min(count.saturating_sub(height));
        if self.selected < self.offset {
            self.offset = self.selected;
        }
        if self.selected >= self.offset + height {
            self.offset = self.selected + 1 - height;
        }
    }
    pub fn move_by(&mut self, delta: isize, count: usize) {
        self.select(self.selected.saturating_add_signed(delta), count);
    }
    pub fn resize(&mut self, area: Rect, count: usize) {
        self.area = area;
        self.select(self.selected, count);
    }
    pub fn row_at(&self, x: u16, y: u16, count: usize) -> Option<usize> {
        let body = Block::bordered().inner(self.area);
        if !body.contains((x, y).into()) {
            return None;
        }
        let index = self.offset + (y - body.y) as usize;
        (index < count).then_some(index)
    }
    pub fn draw(&self, frame: &mut Frame, queues: &[Queue], current: usize, command: usize) {
        let block = Block::bordered()
            .title(" Cues ")
            .border_style(Style::default().fg(if self.focused {
                Color::Cyan
            } else {
                Color::DarkGray
            }))
            .style(Style::default().bg(Color::Rgb(18, 24, 34)));
        let body = block.inner(self.area);
        frame.render_widget(block, self.area);
        if queues.is_empty() {
            frame.render_widget(Paragraph::new("No cues\no: edit demo/add cues"), body);
            return;
        }
        let lines: Vec<_> = queues
            .iter()
            .enumerate()
            .skip(self.offset)
            .take(body.height as usize)
            .map(|(i, cue)| {
                let marker = if i == current { "▶" } else { " " };
                let progress = if i == current {
                    format!(" [{}/{}]", command + 1, cue.commands.len())
                } else {
                    String::new()
                };
                let style = if i == self.selected {
                    Style::default()
                        .fg(Color::White)
                        .bg(if self.focused {
                            Color::Rgb(35, 78, 98)
                        } else {
                            Color::Rgb(38, 46, 58)
                        })
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                Line::styled(format!("{marker} {}. {}{progress}", i + 1, cue.name), style)
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_scrolls_and_mouse_maps_visible_rows() {
        let mut cues = CueList::default();
        cues.resize(Rect::new(0, 4, 26, 6), 20);
        cues.select(10, 20);
        assert_eq!(cues.offset, 7);
        assert_eq!(cues.row_at(2, 5, 20), Some(7));
        assert_eq!(cues.row_at(2, 8, 20), Some(10));
        cues.move_by(-50, 20);
        assert_eq!((cues.selected, cues.offset), (0, 0));
        cues.select(10, 0);
        assert_eq!(cues.row_at(2, 5, 0), None);
    }
}
