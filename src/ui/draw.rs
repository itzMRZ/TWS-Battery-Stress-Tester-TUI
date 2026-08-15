//! Layout for deck, prep, soak, and archive.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Gauge, HighlightSpacing, LineGauge, List, ListItem, ListState, Padding,
    Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Sparkline, Wrap,
};
use ratatui::Frame;

use crate::brand::MarkBox;
use crate::cells;
use crate::death::{Decision, EventKind};
use crate::device::{AncKnowledge, FoundDevice};
use crate::pack::fmt_elapsed;

use super::icons;
use super::theme::{self, Theme};
use super::{App, Notice, NoticeKind, Overlay, Screen};

const MIN_WIDTH: u16 = 64;
const MIN_HEIGHT: u16 = 18;

const WORDMARK: &[&str] = &[r"  ♪  tws-tester"];

pub fn draw(frame: &mut Frame, app: &App) {
    let theme = Theme::mocha();
    let area = frame.area();
    frame.render_widget(Block::default().style(theme.fill()), area);

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area, &theme);
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);

    draw_header(frame, chunks[0], app, &theme);
    match app.screen {
        Screen::Deck => draw_deck(frame, chunks[1], app, &theme),
        Screen::Prep => draw_prep(frame, chunks[1], app, &theme),
        Screen::Soak => draw_soak(frame, chunks[1], app, &theme),
        Screen::Archive => draw_archive(frame, chunks[1], app, &theme),
    }
    draw_footer(frame, chunks[2], app, &theme);

    match &app.overlay {
        Overlay::None => {}
        Overlay::Help => draw_help(frame, area, app, &theme),
        Overlay::ConfirmQuit => confirm(
            frame,
            area,
            &theme,
            "quit?",
            "Leave tws-tester?",
            "Yes",
            "Stay",
        ),
        Overlay::ConfirmStop => confirm(
            frame,
            area,
            &theme,
            "stop soak?",
            "Stop this soak and keep the pack on disk?",
            "Stop",
            "Keep going",
        ),
        Overlay::EditAlias { draft, .. } => draw_alias(frame, area, draft, &theme),
    }

    if let Some(notice) = &app.notice {
        draw_toast(frame, area, notice, &theme);
    }
}

fn draw_too_small(frame: &mut Frame, area: Rect, theme: &Theme) {
    let msg = format!(
        "terminal is {}x{}; resize to at least {MIN_WIDTH}x{MIN_HEIGHT}",
        area.width, area.height
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(msg, theme.warning)))
            .alignment(Alignment::Center)
            .block(Block::default().style(theme.fill())),
        area,
    );
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let screen = match app.screen {
        Screen::Deck => "deck",
        Screen::Prep => "prep",
        Screen::Soak => "soak",
        Screen::Archive => "archive",
    };
    let right = match &app.soak {
        Some(s) if app.screen == Screen::Soak => {
            format!("{}  {}", s.alias, fmt_elapsed(s.elapsed_ms))
        }
        _ => format!("v{}", env!("CARGO_PKG_VERSION")),
    };
    let left = Line::from(vec![
        Span::styled(WORDMARK[0], theme.title),
        Span::styled("    ", theme.muted),
        Span::styled(screen, theme.text_dim),
    ]);
    let cols = Layout::horizontal([
        Constraint::Min(1),
        Constraint::Length(right.len() as u16 + 2),
    ])
    .split(area);
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(theme.border)
        .style(theme.fill());
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(left), cols[0]);
    frame.render_widget(
        Paragraph::new(Span::styled(format!("{right} "), theme.muted)).alignment(Alignment::Right),
        cols[1],
    );
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let items: &[(&str, &str)] = match (&app.overlay, app.screen) {
        (Overlay::Help, _) => &[("Esc", "Close"), ("?", "Close"), ("q", "Quit")],
        (Overlay::ConfirmQuit | Overlay::ConfirmStop, _) => {
            &[("y", "Yes"), ("n", "No"), ("Esc", "No")]
        }
        (Overlay::EditAlias { .. }, _) => &[("Enter", "Save"), ("Esc", "Cancel")],
        (_, Screen::Deck) => &[
            ("↑↓", "Move"),
            ("Enter", "Prep"),
            ("a", "Alias"),
            ("p", "Packs"),
            ("o", "Folder"),
            ("?", "Help"),
            ("q", "Quit"),
        ],
        (_, Screen::Prep) => &[
            ("s", "Stimulus"),
            ("[", "Vol−"),
            ("]", "Vol+"),
            ("c", "Codec"),
            ("Enter", "Start"),
            ("Esc", "Back"),
        ],
        (_, Screen::Soak) => &[("Esc", "Stop"), ("o", "Folder"), ("?", "Help")],
        (_, Screen::Archive) => &[
            ("↑↓", "Move"),
            ("Space", "Mark"),
            ("v", "Overlay"),
            ("Enter", "Open"),
            ("Esc", "Back"),
            ("?", "Help"),
        ],
    };
    frame.render_widget(
        Paragraph::new(theme::hints(theme, area.width, items)).alignment(Alignment::Center),
        area,
    );
}

fn draw_deck(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    if let Some(err) = app.host_error.as_deref() {
        centered_status(frame, area, theme, "×", theme.error, err);
        return;
    }
    if app.devices.is_empty() {
        draw_empty_deck(frame, area, theme);
        return;
    }
    let body =
        Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)]).split(area);
    draw_deck_list(frame, body[0], app, theme);
    draw_detected(frame, body[1], app, theme);
}

fn draw_empty_deck(frame: &mut Frame, area: Rect, theme: &Theme) {
    let lines = vec![
        Line::from(Span::styled("◎", theme.accent)).centered(),
        Line::from(Span::styled(
            "nothing audio-shaped on bluetooth",
            theme.text,
        ))
        .centered(),
        Line::from(Span::styled(
            "pair a Device, then it shows up here",
            theme.text_dim,
        ))
        .centered(),
    ];
    let card = centered(area, area.width.min(56), 7, 28, 64);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(theme.border_type())
                .border_style(theme.border)
                .style(theme.fill()),
        ),
        card,
    );
}

fn draw_deck_list(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let title = format!(
        "◎  Devices · {}/{}",
        app.selected.saturating_add(1).min(app.devices.len()),
        app.devices.len()
    );
    let block = theme.panel(title, true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mark = list_mark_box(inner.width);
    let name_w = inner
        .width
        .saturating_sub(2 /* pip */)
        .saturating_sub(mark.cols)
        .saturating_sub(1 /* gap */)
        .saturating_sub(6 /* bat+pct */)
        .saturating_sub(2 /* highlight */) as usize;
    let items: Vec<ListItem> = app
        .devices
        .iter()
        .map(|d| device_row(app, d, mark, name_w, theme))
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.selected));
    let list = List::new(items)
        .highlight_style(theme.selection())
        .highlight_symbol("▌ ")
        .highlight_spacing(HighlightSpacing::Always);
    frame.render_stateful_widget(list, inner, &mut state);

    if app.devices.len() > (inner.height as usize / 2).max(1) {
        let mut sb = ScrollbarState::new(app.devices.len()).position(app.selected);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(theme.accent)
                .track_style(Style::default().fg(theme.surface1))
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            inner,
            &mut sb,
        );
    }
}

fn list_mark_box(inner_width: u16) -> MarkBox {
    const PIP: u16 = 2;
    const GAP: u16 = 1;
    const BAT: u16 = 6;
    const HIGHLIGHT: u16 = 2;
    const MIN_NAME: u16 = 8;
    let reserve = PIP + GAP + BAT + HIGHLIGHT + MIN_NAME;
    MarkBox {
        cols: inner_width
            .saturating_sub(reserve)
            .clamp(4, MarkBox::COMPACT.cols),
        rows: MarkBox::COMPACT.rows,
    }
}

fn logo_slot(d: &FoundDevice, mark: MarkBox) -> Vec<String> {
    let rows = d.brand.logo(mark);
    if rows.is_empty() {
        vec![" ".repeat(mark.cols as usize); mark.rows.max(1) as usize]
    } else {
        rows
    }
}

fn device_row<'a>(
    app: &App,
    d: &FoundDevice,
    mark: MarkBox,
    name_w: usize,
    theme: &Theme,
) -> ListItem<'a> {
    let product = d.brand.product_label(&d.name);
    let alias = app.aliases.alias_of(&d.address).unwrap_or(product.as_str());
    let pct = d
        .headline_percent()
        .map(|p| format!("{p:>3}%"))
        .unwrap_or_else(|| "  —".into());
    let pip = icons::link(d.connected);
    let pip_style = if d.connected {
        theme.success
    } else {
        theme.muted
    };
    let (br, bg, bb) = d.brand.ink();
    let brand_ink = Style::default().fg(Color::Rgb(br, bg, bb));
    let pct_style = if d.connected { theme.text } else { theme.muted };
    let bat = icons::battery(d.headline_percent());
    let subtitle = if alias == product {
        format!(
            "{} {} · {}",
            icons::class(d.class),
            if d.connected { "live" } else { "paired" },
            d.class.label()
        )
    } else {
        format!(
            "{} {} · {}",
            icons::class(d.class),
            product,
            if d.connected { "live" } else { "paired" }
        )
    };
    let art = logo_slot(d, mark);
    let mut lines = Vec::with_capacity(art.len());
    for (i, row) in art.into_iter().enumerate() {
        if i == 0 {
            lines.push(Line::from(vec![
                Span::styled(format!("{pip} "), pip_style),
                Span::styled(row, brand_ink),
                Span::styled(format!(" {}", truncate(alias, name_w)), theme.text),
                Span::styled(format!(" {bat}{pct}"), pct_style),
            ]));
        } else if i == 1 {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(row, brand_ink),
                Span::styled(format!(" {subtitle}"), theme.muted),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(row, brand_ink),
            ]));
        }
    }
    ListItem::new(lines)
}

fn draw_detected(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let Some(d) = app.devices.get(app.selected) else {
        return;
    };
    let product = d.brand.product_label(&d.name);
    let alias = app.aliases.alias_of(&d.address).unwrap_or(product.as_str());
    let block = theme
        .panel("▣ Detected", false)
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mark =
        MarkBox::fit(inner.width, inner.height.saturating_sub(9)).filter(|_| d.brand.has_logo());
    let art = mark.map(|m| d.brand.logo(m)).unwrap_or_default();
    let show_art = !art.is_empty();
    let body = if show_art {
        Layout::vertical([Constraint::Length(art.len() as u16 + 1), Constraint::Min(1)])
            .split(inner)
    } else {
        Layout::vertical([Constraint::Length(0), Constraint::Min(1)]).split(inner)
    };

    if show_art {
        let (r, g, b) = d.brand.ink();
        let ink = Style::default().fg(Color::Rgb(r, g, b));
        let lines: Vec<Line> = art
            .iter()
            .map(|row| Line::from(Span::styled(row.clone(), ink)))
            .collect();
        frame.render_widget(Paragraph::new(lines), body[0]);
    }

    let mut link = if d.connected {
        "live".to_string()
    } else {
        "paired".into()
    };
    if d.bonded {
        link.push_str("  bonded");
    }

    let mut lines = vec![Line::from(Span::styled(product.clone(), theme.title))];
    if alias != product {
        fact(&mut lines, theme, "alias", alias);
    }
    fact(&mut lines, theme, "name", &d.name);
    fact(&mut lines, theme, "address", &d.address);
    fact(&mut lines, theme, "link", &link);
    fact(&mut lines, theme, "cells", &cells_line(d));
    let srcs: Vec<&str> = unique_sources(d);
    if !srcs.is_empty() {
        fact(&mut lines, theme, "source", &srcs.join("  "));
    }
    fact(&mut lines, theme, "codec", &d.pretty_codec());
    if !d.codec_choices().is_empty() {
        fact(&mut lines, theme, "codecs", &d.codec_choices().join("  "));
    }
    if let Some(v) = d.host_volume {
        fact(&mut lines, theme, "volume", &format!("host {v}%"));
    }
    let services = d.services();
    if !services.is_empty() {
        fact(&mut lines, theme, "services", &services.join("  "));
    }
    fact(&mut lines, theme, "anc", &anc_label(d));
    if let Some(chip) = &d.chip {
        fact(&mut lines, theme, "chip", chip);
    }
    if let Some(rssi) = d.rssi {
        fact(&mut lines, theme, "rssi", &format!("{rssi} dBm"));
    }

    let value_w = body[1].width.saturating_sub(12) as usize;
    for line in &mut lines {
        if let Some(last) = line.spans.last_mut() {
            let s = last.content.to_string();
            if s.chars().count() > value_w {
                last.content = truncate(&s, value_w).into();
            }
        }
    }
    frame.render_widget(Paragraph::new(lines), body[1]);
}

fn unique_sources(d: &FoundDevice) -> Vec<&str> {
    let mut v = Vec::new();
    for c in &d.cells {
        if !v.contains(&c.source.as_str()) {
            v.push(c.source.as_str());
        }
    }
    v
}

fn anc_label(d: &FoundDevice) -> String {
    match &d.anc {
        AncKnowledge::Unknown => "not readable on this stack".into(),
        AncKnowledge::Known { mode, can_set } => {
            if *can_set {
                mode.label().to_string()
            } else {
                format!("{}  (read only)", mode.label())
            }
        }
    }
}

fn fact(lines: &mut Vec<Line<'static>>, theme: &Theme, key: &'static str, value: &str) {
    lines.push(Line::from(vec![
        Span::styled(format!("{key:<9}"), theme.muted),
        Span::styled(value.to_string(), theme.text),
    ]));
}

fn draw_prep(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let Some(d) = app.devices.get(app.selected) else {
        return;
    };
    let cols =
        Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)]).split(area);
    draw_prep_device(frame, cols[0], app, d, theme);
    draw_prep_condition(frame, cols[1], app, d, theme);
}

fn draw_prep_device(frame: &mut Frame, area: Rect, app: &App, d: &FoundDevice, theme: &Theme) {
    let product = d.brand.product_label(&d.name);
    let alias = app.aliases.alias_of(&d.address).unwrap_or(product.as_str());
    let kind = crate::device::SoakKind::from_start_percent(d.headline_percent());
    let block = theme.panel(alias, false).padding(Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let art = MarkBox::fit(inner.width, inner.height.saturating_sub(8))
        .filter(|_| d.brand.has_logo())
        .map(|m| d.brand.logo(m))
        .unwrap_or_default();
    let (r, g, b) = d.brand.ink();
    let ink = Style::default().fg(Color::Rgb(r, g, b));
    let mut lines: Vec<Line> = art
        .iter()
        .map(|row| Line::from(Span::styled(row.clone(), ink)))
        .collect();
    if !art.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(product, theme.title)));
    lines.push(Line::from(Span::styled(d.address.clone(), theme.muted)));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("cells   {}", cells_line(d)),
        theme.text,
    )));
    lines.push(Line::from(Span::styled(
        format!("anc     {}", anc_label(d)),
        theme.text_dim,
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("pack    ", theme.muted),
        Span::styled(kind.label().to_string(), theme.text),
    ]));
    if matches!(kind, crate::device::SoakKind::Remaining) {
        lines.push(Line::from(Span::styled(
            "starts below ~full: remaining, not full life",
            theme.warning,
        )));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn draw_prep_condition(frame: &mut Frame, area: Rect, app: &App, d: &FoundDevice, theme: &Theme) {
    let block = theme
        .panel("Condition", true)
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let vol = app.prep_volume.unwrap_or(d.host_volume.unwrap_or(50));
    let codec = app
        .prep_codec
        .as_deref()
        .map(|k| d.pretty_profile(k))
        .unwrap_or_else(|| d.pretty_codec());
    let rows = [
        ("stimulus", app.prep_stimulus.label().to_string()),
        ("volume", format!("host {vol}%")),
        ("codec", codec),
    ];
    let mut y = inner.y;
    for (i, (key, value)) in rows.iter().enumerate() {
        if y >= inner.bottom() {
            break;
        }
        let sel = i == app.prep_field;
        let row = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        let style = if sel { theme.selection() } else { theme.fill() };
        frame.render_widget(Block::default().style(style), row);
        let mark = if sel { "▸ " } else { "  " };
        let key_icon = match *key {
            "stimulus" => "♪",
            "volume" => "▁",
            "codec" => "◈",
            _ => "·",
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(mark, if sel { theme.accent } else { theme.muted }),
                Span::styled(format!("{key_icon} {key:<8}"), theme.muted),
                Span::styled(
                    value.clone(),
                    if sel {
                        theme.text.add_modifier(Modifier::BOLD)
                    } else {
                        theme.text
                    },
                ),
            ])),
            row,
        );
        y += 1;
        if i == 1 && y < inner.bottom() {
            let gauge = Rect {
                x: inner.x.saturating_add(2),
                y,
                width: inner.width.saturating_sub(2),
                height: 1,
            };
            frame.render_widget(
                LineGauge::default()
                    .ratio(f64::from(vol) / 100.0)
                    .label("")
                    .filled_style(Style::default().fg(theme.cell_color(Some(vol))))
                    .unfilled_style(Style::default().fg(theme.surface1)),
                gauge,
            );
            y += 1;
        }
    }
    if app.prep_stimulus == crate::device::Stimulus::Playlist && y + 1 < inner.bottom() {
        let n = app.playlist.len();
        let cur = app
            .playlist
            .get(app.playlist_idx)
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "no files in Music".into());
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(
                    "  {}  ({n})",
                    truncate(&cur, inner.width.saturating_sub(8) as usize)
                ),
                theme.text_dim,
            )),
            Rect {
                x: inner.x,
                y: y.saturating_add(1),
                width: inner.width,
                height: 1,
            },
        );
    }
}

fn draw_soak(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let Some(s) = &app.soak else { return };
    let top = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);
    draw_soak_status(frame, top[0], s, theme);

    let body = if area.width >= 72 {
        Layout::horizontal([Constraint::Length(32), Constraint::Min(20)]).split(top[1])
    } else {
        Layout::vertical([Constraint::Length(9), Constraint::Min(4)]).split(top[1])
    };
    draw_soak_cells(frame, body[0], s, theme);
    let right = Layout::vertical([Constraint::Length(9), Constraint::Min(3)]).split(body[1]);
    draw_soak_timeline(frame, right[0], s, theme);
    draw_soak_events(frame, right[1], s, theme);
}

fn draw_soak_status(frame: &mut Frame, area: Rect, s: &super::LiveSoak, theme: &Theme) {
    let (mark, label, style) = match s.decision {
        Decision::Live => ("▶", "playing", theme.success),
        Decision::Confirming => ("●", "waiting (might be dead)", theme.warning),
        Decision::Dead => ("■", "dead", theme.error),
        Decision::Interrupted => ("■", "stopped", theme.text_dim),
    };
    let has_sink = s.device.sink.is_some() && s.device.connected;
    let sink = if has_sink { "▷ sink" } else { "× no sink" };
    let block = theme.panel("▶ Status", true);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {mark} {label}  "),
                style.add_modifier(Modifier::BOLD),
            ),
            Span::styled("·  ", theme.muted),
            Span::styled(s.kind.label().to_string(), theme.text),
            Span::styled("  ·  ", theme.muted),
            Span::styled(s.stimulus.label().to_string(), theme.text),
            Span::styled("  ·  ", theme.muted),
            Span::styled(s.device.pretty_codec(), theme.text),
            Span::styled("  ·  ", theme.muted),
            Span::styled(
                sink,
                if has_sink {
                    theme.text_dim
                } else {
                    theme.error
                },
            ),
        ])),
        inner,
    );
}

fn draw_soak_cells(frame: &mut Frame, area: Rect, s: &super::LiveSoak, theme: &Theme) {
    let pct = s
        .samples
        .last()
        .and_then(|x| cells::headline(&x.cells))
        .or(s.device.headline_percent());
    let block = theme.panel("▮ Cells", false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let parts = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(inner);
    let label = pct.map(|p| format!("{p}%")).unwrap_or_else(|| "—".into());
    frame.render_widget(
        Gauge::default()
            .percent(u16::from(pct.unwrap_or(0)))
            .label(label)
            .gauge_style(
                Style::default()
                    .fg(theme.cell_color(pct))
                    .bg(theme.surface0),
            )
            .style(theme.text.add_modifier(Modifier::BOLD)),
        parts[0],
    );
    if s.device.cells.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(" no cell yet", theme.text_dim)),
            parts[1],
        );
        return;
    }
    for (c, y) in s.device.cells.iter().zip(parts[1].y..parts[1].bottom()) {
        let ratio = f64::from(c.percent.unwrap_or(0)) / 100.0;
        let label = format!(
            "{} {} {}",
            icons::cell(&c.name),
            c.name,
            c.percent
                .map(|p| format!("{p}%"))
                .unwrap_or_else(|| "—".into())
        );
        frame.render_widget(
            LineGauge::default()
                .ratio(ratio)
                .label(label)
                .filled_style(Style::default().fg(theme.cell_color(c.percent)))
                .unfilled_style(Style::default().fg(theme.surface1)),
            Rect {
                x: parts[1].x,
                y,
                width: parts[1].width,
                height: 1,
            },
        );
    }
}

fn spark_data(samples: &[crate::device::Sample], name: &str) -> Vec<u64> {
    samples
        .iter()
        .filter_map(|x| cells::named_percent(&x.cells, name))
        .map(u64::from)
        .collect()
}

fn draw_soak_timeline(frame: &mut Frame, area: Rect, s: &super::LiveSoak, theme: &Theme) {
    let mut series: Vec<(&str, Vec<u64>, Style)> = Vec::new();
    for (name, style) in [
        ("left", theme.accent),
        ("right", theme.success),
        ("pair", theme.text),
    ] {
        let data = spark_data(&s.samples, name);
        if !data.is_empty() {
            series.push((name, data, style));
        }
    }
    if series.is_empty() {
        let data: Vec<u64> = s
            .samples
            .iter()
            .filter_map(|x| cells::headline(&x.cells))
            .map(u64::from)
            .collect();
        if !data.is_empty() {
            series.push(("cells", data, theme.accent));
        }
    }
    let block = theme.panel("▁ Timeline", false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if series.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " waiting for the first sample",
                theme.text_dim,
            )),
            inner,
        );
        return;
    }
    let rows = Layout::vertical(
        series
            .iter()
            .map(|_| Constraint::Min(2))
            .collect::<Vec<_>>(),
    )
    .split(inner);
    for ((name, data, style), row) in series.iter().zip(rows.iter()) {
        let label_w = 8u16;
        let cols =
            Layout::horizontal([Constraint::Length(label_w), Constraint::Min(4)]).split(*row);
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" {} {name}", icons::cell(name)),
                *style,
            )),
            cols[0],
        );
        frame.render_widget(
            Sparkline::default().data(data).max(100).style(*style),
            cols[1],
        );
    }
}

fn draw_soak_events(frame: &mut Frame, area: Rect, s: &super::LiveSoak, theme: &Theme) {
    let block = theme
        .panel("· Events", false)
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if s.events.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("none yet", theme.text_dim)),
            inner,
        );
        return;
    }
    let keep = inner.height as usize;
    let lines: Vec<Line> = s
        .events
        .iter()
        .rev()
        .take(keep)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|(ms, e)| event_line(*ms, *e, theme))
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn event_line(ms: u64, e: EventKind, theme: &Theme) -> Line<'static> {
    let tag_style = match e {
        EventKind::Dead | EventKind::ReportedEmptyStillPlaying => theme.error,
        EventKind::FalseDeath | EventKind::PercentStuck | EventKind::Confirming => theme.warning,
        EventKind::Blip => theme.accent,
        EventKind::Interrupted => theme.text_dim,
    };
    Line::from(vec![
        Span::styled(format!("{:>8}  ", fmt_elapsed(ms)), theme.muted),
        Span::styled(format!("[{}] ", e.tag()), tag_style),
        Span::styled(e.to_string(), theme.text),
    ])
}

fn draw_archive(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let alias = app.archive_alias.as_deref().unwrap_or("?");
    let n = app.archive_items.len();
    let title = if n == 0 {
        format!("{alias}  ·  no soaks yet")
    } else {
        format!(
            "{alias}  ·  {}/{}",
            app.archive_sel.saturating_add(1).min(n),
            n
        )
    };
    let block = theme.panel(title, true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.archive_items.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " no soaks yet; start one from prep",
                theme.text_dim,
            )),
            inner,
        );
        return;
    }

    let items: Vec<ListItem> = app
        .archive_items
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let mark = if app.archive_marks.contains(&i) {
                "●"
            } else {
                "·"
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{mark}  "), theme.accent),
                Span::styled(name.clone(), theme.text),
            ]))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.archive_sel));
    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(theme.selection())
            .highlight_symbol("▌ ")
            .highlight_spacing(HighlightSpacing::Always),
        inner,
        &mut state,
    );
}

fn draw_help(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let mut lines = vec![
        help_heading(theme, "Global"),
        help_row(theme, "?", "Toggle this overlay"),
        help_row(theme, "q", "Quit"),
        help_row(theme, "Esc", "Back / close"),
        Line::from(""),
    ];
    match app.screen {
        Screen::Deck => {
            lines.push(help_heading(theme, "Deck"));
            lines.push(help_row(theme, "↑ ↓  j k", "Move"));
            lines.push(help_row(
                theme,
                "Enter  l  Space",
                "Prep the selected Device",
            ));
            lines.push(help_row(theme, "a", "Rename the Alias"));
            lines.push(help_row(theme, "p", "Open packs for this Device"));
            lines.push(help_row(theme, "o", "Open the Device folder"));
        }
        Screen::Prep => {
            lines.push(help_heading(theme, "Prep"));
            lines.push(help_row(theme, "s", "Cycle stimulus"));
            lines.push(help_row(theme, "[  ]   -  =", "Host volume"));
            lines.push(help_row(theme, "c", "Cycle codec"));
            lines.push(help_row(theme, "↑ ↓  j k", "Highlight a row"));
            lines.push(help_row(theme, "← →  h l", "Change the highlighted row"));
            lines.push(help_row(
                theme,
                "1  2  3",
                "Jump to stimulus / volume / codec",
            ));
            lines.push(help_row(theme, "Enter  Space", "Start the Soak"));
        }
        Screen::Soak => {
            lines.push(help_heading(theme, "Soak"));
            lines.push(help_row(theme, "Esc", "Stop; pack stays on disk"));
            lines.push(help_row(theme, "o", "Open the support pack folder"));
        }
        Screen::Archive => {
            lines.push(help_heading(theme, "Archive"));
            lines.push(help_row(theme, "↑ ↓  j k", "Move"));
            lines.push(help_row(theme, "Space", "Mark two soaks"));
            lines.push(help_row(theme, "v", "Write overlay.html for the marks"));
            lines.push(help_row(theme, "Enter  o", "Open report.html"));
            lines.push(help_row(theme, "Esc  h", "Back to deck"));
        }
    }
    let width = lines.iter().map(Line::width).max().unwrap_or(42) as u16 + 4;
    let height = lines.len() as u16 + 2;
    let popup = centered(area, width, height, 46, 72);
    clear_modal(frame, area, popup, theme);
    frame.render_widget(
        Paragraph::new(lines).block(theme.popup("Keybindings")),
        popup,
    );
}

fn help_heading(theme: &Theme, title: &str) -> Line<'static> {
    Line::from(Span::styled(format!(" {title}"), theme.title))
}

fn help_row(theme: &Theme, key: &str, action: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" [{key}] "), theme.accent),
        Span::styled(action.to_string(), theme.text),
    ])
}

fn confirm(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    title: &str,
    summary: &str,
    yes: &str,
    no: &str,
) {
    let popup = centered(area, 48, 7, 36, 56);
    clear_modal(frame, area, popup, theme);
    let block = theme.popup(title);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);
    frame.render_widget(
        Paragraph::new(Span::styled(summary.to_string(), theme.text)).alignment(Alignment::Center),
        rows[0],
    );
    let actions = Line::from(vec![
        Span::styled(format!(" {yes} "), theme.selection()),
        Span::raw("   "),
        Span::styled(format!(" {no} "), theme.text_dim),
    ]);
    frame.render_widget(
        Paragraph::new(actions).alignment(Alignment::Center),
        rows[2],
    );
}

fn draw_alias(frame: &mut Frame, area: Rect, draft: &str, theme: &Theme) {
    let popup = centered(area, 48, 5, 36, 60);
    clear_modal(frame, area, popup, theme);
    let block = theme.popup("alias");
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(draft.to_string(), theme.text.add_modifier(Modifier::BOLD)),
            Span::styled("█", theme.accent),
        ]))
        .alignment(Alignment::Center),
        inner,
    );
}

fn draw_toast(frame: &mut Frame, area: Rect, notice: &Notice, theme: &Theme) {
    let (symbol, style) = match notice.kind {
        NoticeKind::Info => ("i", theme.accent),
        NoticeKind::Success => ("✓", theme.success),
        NoticeKind::Warning => ("!", theme.warning),
        NoticeKind::Error => ("×", theme.error),
    };
    let message = truncate(&notice.message, 48);
    let width = (notice.title.len() + 8)
        .max(message.len() + 4)
        .clamp(24, 52)
        .min(area.width.saturating_sub(4) as usize) as u16;
    let toast = Rect::new(
        area.right().saturating_sub(width).saturating_sub(2),
        area.bottom().saturating_sub(6),
        width,
        3,
    );
    frame.render_widget(Clear, toast);
    frame.render_widget(Block::default().style(theme.fill()), toast);
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(format!(" {symbol} "), style),
            Span::styled(notice.title.clone(), style.add_modifier(Modifier::BOLD)),
            Span::raw(" "),
        ]))
        .borders(Borders::ALL)
        .border_type(theme.border_type())
        .border_style(style)
        .padding(Padding::horizontal(1));
    frame.render_widget(
        Paragraph::new(message)
            .style(theme.text)
            .wrap(Wrap { trim: true })
            .block(block),
        toast,
    );
}

fn centered_status(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    symbol: &str,
    style: Style,
    message: &str,
) {
    let card = centered(area, area.width.min(64), 5, 24, 72);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(theme.border_type())
        .border_style(theme.border)
        .style(theme.fill());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{symbol} "), style),
            Span::styled(message.to_string(), theme.text),
        ]))
        .alignment(Alignment::Center)
        .block(block),
        card,
    );
}

fn cells_line(d: &FoundDevice) -> String {
    if d.cells.is_empty() {
        return "no cell yet".into();
    }
    d.cells
        .iter()
        .map(|c| {
            format!(
                "{} {} {}",
                icons::cell(&c.name),
                c.name,
                c.percent
                    .map(|p| format!("{p}%"))
                    .unwrap_or_else(|| "—".into())
            )
        })
        .collect::<Vec<_>>()
        .join("   ")
}

fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    if max == 1 {
        return "…".into();
    }
    format!("{}…", s.chars().take(max - 1).collect::<String>())
}

fn centered(
    area: Rect,
    desired_width: u16,
    desired_height: u16,
    minimum_width: u16,
    maximum_width: u16,
) -> Rect {
    let available_width = area.width.saturating_sub(2).max(1);
    let available_height = area.height.saturating_sub(2).max(1);
    let width = desired_width
        .max(minimum_width.min(available_width))
        .min(maximum_width)
        .min(available_width);
    let height = desired_height.min(available_height).max(1);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn clear_modal(frame: &mut Frame, bounds: Rect, popup: Rect, theme: &Theme) {
    let x = popup.x.saturating_sub(3).max(bounds.x);
    let y = popup.y.saturating_sub(1).max(bounds.y);
    let right = popup.right().saturating_add(3).min(bounds.right());
    let bottom = popup.bottom().saturating_add(1).min(bounds.bottom());
    let halo = Rect::new(x, y, right.saturating_sub(x), bottom.saturating_sub(y));
    frame.render_widget(Clear, halo);
    frame.render_widget(Block::default().style(theme.fill()), halo);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_popup_sits_in_the_middle() {
        let area = Rect::new(0, 0, 80, 24);
        let r = centered(area, 40, 8, 20, 60);
        assert_eq!(r, Rect::new(20, 8, 40, 8));
    }

    #[test]
    fn truncate_keeps_short_strings() {
        assert_eq!(truncate("P30i", 8), "P30i");
        assert_eq!(truncate("soundcore P30i", 10), "soundcore…");
    }
}
