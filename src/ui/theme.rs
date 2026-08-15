use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders};

/// Catppuccin Mocha. Accent is blue; green / peach / red are live / low cell / dead.
pub struct Theme {
    pub border: Style,
    pub border_focus: Style,
    pub text: Style,
    pub text_dim: Style,
    pub title: Style,
    pub error: Style,
    pub success: Style,
    pub warning: Style,
    pub shortcut: Style,
    pub accent: Style,
    pub muted: Style,
    pub surface0: Color,
    pub surface1: Color,
    pub base: Color,
    pub rounded: bool,
}

impl Theme {
    pub fn mocha() -> Self {
        Self {
            border: Style::default().fg(rgb(69, 71, 90)),
            border_focus: Style::default().fg(rgb(137, 180, 250)),
            text: Style::default().fg(rgb(205, 214, 244)),
            text_dim: Style::default().fg(rgb(166, 173, 200)),
            title: Style::default()
                .fg(rgb(205, 214, 244))
                .add_modifier(Modifier::BOLD),
            error: Style::default().fg(rgb(243, 139, 168)),
            success: Style::default().fg(rgb(166, 227, 161)),
            warning: Style::default().fg(rgb(250, 179, 135)),
            shortcut: Style::default().fg(rgb(137, 180, 250)),
            accent: Style::default().fg(rgb(137, 180, 250)),
            muted: Style::default().fg(rgb(108, 112, 134)),
            surface0: rgb(49, 50, 68),
            surface1: rgb(69, 71, 90),
            base: rgb(30, 30, 46),
            rounded: !basic_terminal(),
        }
    }

    pub fn fill(&self) -> Style {
        Style::default().bg(self.base)
    }

    pub fn selection(&self) -> Style {
        self.text.add_modifier(Modifier::BOLD).bg(self.surface0)
    }

    pub fn border_type(&self) -> BorderType {
        if self.rounded {
            BorderType::Rounded
        } else {
            BorderType::Plain
        }
    }

    pub fn panel(&self, title: impl AsRef<str>, focus: bool) -> Block<'static> {
        Block::default()
            .title(format!(" {} ", title.as_ref()))
            .title_style(if focus {
                self.accent.add_modifier(Modifier::BOLD)
            } else {
                self.text_dim
            })
            .borders(Borders::ALL)
            .border_type(self.border_type())
            .border_style(if focus {
                self.border_focus
            } else {
                self.border
            })
            .style(self.fill().fg(self.fg()))
    }

    pub fn popup(&self, title: impl AsRef<str>) -> Block<'static> {
        Block::default()
            .title(format!(" {} ", title.as_ref()))
            .title_style(self.title)
            .title_alignment(ratatui::layout::Alignment::Center)
            .borders(Borders::ALL)
            .border_type(self.border_type())
            .border_style(self.border_focus)
            .style(self.fill().fg(self.fg()))
    }

    pub fn fg(&self) -> Color {
        self.text.fg.unwrap_or(Color::White)
    }

    /// Accent when healthy, peach under 20%, red at 0%.
    pub fn cell_color(&self, percent: Option<u8>) -> Color {
        match percent {
            None => rgb(108, 112, 134),
            Some(0) => rgb(243, 139, 168),
            Some(n) if n < 20 => rgb(250, 179, 135),
            _ => rgb(137, 180, 250),
        }
    }

    pub fn key_hint(&self, key: &str, action: &str) -> Vec<Span<'static>> {
        vec![
            Span::styled("[", self.muted),
            Span::styled(key.to_string(), self.shortcut),
            Span::styled("] ", self.muted),
            Span::styled(action.to_string(), self.text_dim),
            Span::raw("  "),
        ]
    }
}

pub fn hints(theme: &Theme, width: u16, items: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    let mut used = 0usize;
    let max = width.saturating_sub(2) as usize;
    for (i, (key, action)) in items.iter().enumerate() {
        let w = key.len() + action.len() + 5;
        if i > 0 && used + w > max {
            break;
        }
        spans.extend(theme.key_hint(key, action));
        used += w;
    }
    Line::from(spans)
}

fn basic_terminal() -> bool {
    let term = std::env::var("TERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    term == "dumb" || term == "linux" || term.contains("fbterm")
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_drop_the_tail_when_narrow() {
        let theme = Theme::mocha();
        let line = hints(
            &theme,
            22,
            &[("↑↓", "Move"), ("Enter", "Prep"), ("?", "Help")],
        );
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("Move"), "{text}");
        assert!(!text.contains("Help"), "{text}");
    }
}
