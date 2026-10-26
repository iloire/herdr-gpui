//! The palette paints prepared entries; input and daemon operations live elsewhere.

use super::{Action, AgentFilter, Filter, Palette, Scope, preview};
use crate::HerdrWindow;
use gpui::{prelude::*, *};

/// The narrowest window, less its margins, that fits Go To's list and
/// preview side by side; a narrower one shows the list alone.
const PREVIEW_MIN_WIDTH: f32 = 840.;

const HINTS: &str = "↑↓ / Tab / ⌃N ⌃P move · ⌃Tab filter · ↵ select · Esc close";
/// Projects are the folders inside the configured roots, so without one
/// the picker has nothing to list.
const NO_PROJECT_ROOTS: &str = "No project folders yet. List the folders that hold your \
     checkouts under [palette] project_roots in the config, such as \
     project_roots = [\"~/code\"], and each folder inside them shows here.";
const OPEN_PROJECT_HINTS: &str = "↑↓ move · ↵ open · Esc close";
const GO_TO_HINTS: &str =
    "↑↓ / Tab / ⌃N ⌃P move · ⌃Tab filter · ⌥B/W/I/D/A status · ⌥P preview · ↵ select · Esc close";

/// How much larger than the interface text the palette's query is drawn.
const SEARCH_SCALE: f32 = 1.5;

/// The interface font, enlarged for the palette's query field.
pub(super) fn search_font(ui: &crate::config::FontConfig) -> crate::config::FontConfig {
    crate::config::FontConfig {
        size: ui.size * SEARCH_SCALE,
        ..ui.clone()
    }
}

impl HerdrWindow {
    /// The query is the palette's focus, as in Raycast: a large frameless
    /// field spans the top, with the filters and count quieter beneath it.
    fn palette_header(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        div()
            .flex_none()
            .child(
                div()
                    .px(px(20.))
                    .py(px(16.))
                    .border_b_1()
                    .border_color(rgb(theme.active))
                    .child(palette.search.clone()),
            )
            .child(
                div()
                    .px(px(14.))
                    .pt(px(8.))
                    .pb(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .text_color(rgb(theme.muted))
                    .when(palette.scope == Scope::OpenProject, |row| {
                        row.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .px_2()
                                .py_1()
                                .text_color(rgb(theme.foreground))
                                .child("Open project"),
                        )
                    })
                    .when(palette.scope == Scope::Everything, |row| {
                        row.child(self.palette_filters(palette, cx))
                    })
                    .child(div().flex_none().child(format!(
                        "{} of {}",
                        palette.filtered.len(),
                        palette.entries.len()
                    ))),
            )
            .when(palette.filter == Filter::Navigation, |header| {
                header.child(
                    div()
                        .px(px(14.))
                        .child(self.palette_agent_filters(palette, cx)),
                )
            })
    }

    /// The filter tabs of the palette opened on everything.
    fn palette_filters(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_wrap()
            .gap(px(4.))
            .children(Filter::ALL.into_iter().map(|filter| {
                div()
                    .id(("palette-filter", filter as usize))
                    .debug_selector(move || {
                        format!("palette-filter-{}", filter.label().to_lowercase())
                    })
                    .px_2()
                    .py_1()
                    .rounded(px(crate::config::corners::CONTROL))
                    .cursor_pointer()
                    .when(filter == palette.filter, |tab| {
                        tab.bg(rgb(theme.active)).text_color(rgb(theme.foreground))
                    })
                    .hover(|tab| tab.bg(rgb(theme.active)))
                    .child(filter.label())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_palette_filter(filter, cx);
                        if let Some(palette) = &this.menu.palette {
                            let focus = palette.search.read(cx).focus.clone();
                            window.focus(&focus, cx);
                        }
                    }))
            }))
    }

    /// Go To's status chips; Alt with the chip's navigator letter picks one
    /// from the keyboard.
    fn palette_agent_filters(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        div()
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .pt(px(6.))
            .text_color(rgb(theme.muted))
            .children(AgentFilter::ALL.into_iter().map(|filter| {
                div()
                    .id(("palette-agent-filter", filter as usize))
                    .debug_selector(move || {
                        format!(
                            "palette-agent-filter-{}",
                            filter.label().to_lowercase().replace(' ', "-")
                        )
                    })
                    .px_2()
                    .py_1()
                    .rounded(px(crate::config::corners::CONTROL))
                    .cursor_pointer()
                    .when(filter == palette.agent_filter, |chip| {
                        chip.bg(rgb(theme.active)).text_color(rgb(theme.foreground))
                    })
                    .hover(|chip| chip.bg(rgb(theme.active)))
                    .child(filter.label())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_palette_agent_filter(filter, cx);
                        if let Some(palette) = &this.menu.palette {
                            let focus = palette.search.read(cx).focus.clone();
                            window.focus(&focus, cx);
                        }
                    }))
            }))
    }

    fn palette_row(
        &self,
        palette: &Palette,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let hit = &palette.filtered[index];
        let entry = &palette.entries[hit.index];
        // Ranked results need not keep entry order, so only an empty query nests.
        let depth = if palette.query.trim().is_empty() {
            hit.depth
        } else {
            0
        };
        let action = entry.action.clone();
        // Matched characters were found while ranking; painting only styles them.
        let matched = HighlightStyle {
            color: Some(rgb(self.theme.primary()).into()),
            font_weight: Some(FontWeight::BOLD),
            ..Default::default()
        };
        let styled = |text: &SharedString, ranges: &[std::ops::Range<usize>]| {
            StyledText::new(text.clone())
                .with_highlights(ranges.iter().map(|range| (range.clone(), matched)))
        };
        div()
            .id(index)
            .debug_selector(move || format!("palette-row-{index}"))
            .w_full()
            .h(px(self.config.ui.line_height() * 2. + 20.))
            .px(px(16.))
            .flex()
            .items_center()
            .gap(px(12.))
            .cursor_pointer()
            .when(index == palette.selected, |row| {
                row.bg(rgb(self.theme.active))
            })
            .hover(|s| s.bg(rgb(self.theme.active)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .pl(px(16. * depth as f32))
                    .child(
                        div()
                            .truncate()
                            .child(styled(&entry.label, &hit.highlights.label)),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_color(rgb(self.theme.muted))
                            .child(styled(&entry.detail, &hit.highlights.detail)),
                    ),
            )
            .when(!entry.badge.is_empty(), |row| {
                row.child(
                    div()
                        .flex_none()
                        .text_color(rgb(self.theme.muted))
                        .child(entry.badge.clone()),
                )
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.activate_palette(action.clone(), window, cx)
            }))
    }

    /// Whether the palette is drawn wide enough for Go To's preview: it is
    /// toggled on, the palette is opened on everything, and the window has room.
    /// Every filter shares the width, so the panel holds still as they change.
    pub(crate) fn palette_wide(&self, viewport: Size<Pixels>) -> bool {
        self.menu.palette_preview
            && self
                .menu
                .palette
                .as_ref()
                .is_some_and(|palette| palette.scope == Scope::Everything)
            && f32::from(viewport.width) - 32. >= PREVIEW_MIN_WIDTH
    }

    /// Whether Go To shows its preview beside the list.
    fn palette_previews(&self, viewport: Size<Pixels>) -> bool {
        self.palette_wide(viewport)
            && self
                .menu
                .palette
                .as_ref()
                .is_some_and(|palette| palette.filter == Filter::Navigation)
    }

    /// The preview of the selected Go To row, from what its host's
    /// connection already holds.
    pub(super) fn palette_preview(&self, palette: &Palette) -> Option<preview::Preview> {
        let Action::Go {
            endpoint,
            boot,
            target,
        } = &palette.selected_entry()?.action
        else {
            return None;
        };
        let index = self.endpoints.iter().position(|e| e.id == *endpoint)?;
        let live = if index == self.selected_endpoint {
            &self.live
        } else {
            &self.endpoints[index].live
        };
        preview::preview(
            live.snapshot.as_deref(),
            live.surface.as_ref(),
            &self.endpoints[index].last_seen,
            boot,
            target.as_deref(),
        )
    }

    fn render_palette_preview(&self, palette: &Palette) -> Div {
        let theme = &self.theme;
        let pane = div()
            .debug_selector(|| "palette-preview".into())
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .border_l_1()
            .border_color(rgb(theme.active));
        match self.palette_preview(palette) {
            Some(preview::Preview::Screen {
                surface,
                rect,
                seen_at,
            }) => {
                let terminal = &self.config.terminal;
                let look = preview::Look {
                    font: terminal.font(),
                    size: terminal.size,
                    line_ratio: terminal.line_height() / terminal.size,
                    cell_ratio: self.cell_width / terminal.size,
                    theme: theme.clone(),
                };
                pane.relative()
                    .bg(rgb(theme.background))
                    .child(preview::paint_screen(
                        surface,
                        rect,
                        palette.preview_painter.clone(),
                        look,
                    ))
                    .when_some(seen_at, |pane, at| {
                        let age = crate::checkpoint::age(
                            i64::try_from(at).unwrap_or(i64::MAX),
                            i64::try_from(super::frecency::now()).unwrap_or(i64::MAX),
                        );
                        pane.child(
                            div()
                                .debug_selector(|| "palette-preview-age".into())
                                .absolute()
                                .top(px(8.))
                                .right(px(8.))
                                .px(px(8.))
                                .py(px(2.))
                                .rounded(px(4.))
                                .bg(rgb(theme.surface))
                                .border_1()
                                .border_color(rgb(theme.active))
                                .text_color(rgb(theme.muted))
                                .child(format!("Last seen {age}")),
                        )
                    })
            }
            Some(preview::Preview::Facts { title, facts }) => pane
                .p(px(16.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .children(facts.into_iter().map(|(name, value)| {
                    div()
                        .flex()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex_none()
                                .w(px(self.config.ui.size * 6.))
                                .text_color(rgb(theme.muted))
                                .child(name),
                        )
                        .child(div().flex_1().min_w_0().truncate().child(value))
                }))
                .child(
                    div()
                        .pt(px(10.))
                        .text_color(rgb(theme.muted))
                        .child("Not on screen. Press Enter to go there and watch it live."),
                ),
            None => pane,
        }
    }

    pub(crate) fn render_palette(&self, viewport: Size<Pixels>, cx: &mut Context<Self>) -> Div {
        let Some(palette) = &self.menu.palette else {
            return div();
        };
        let theme = &self.theme;
        let previews = self.palette_previews(viewport);
        let no_roots =
            palette.scope == Scope::OpenProject && self.config.palette.project_roots.is_empty();
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(self.palette_header(palette, cx))
            .when(palette.loading_projects, |panel| {
                panel.child(
                    div()
                        .flex_none()
                        .px(px(16.))
                        .py(px(8.))
                        .text_color(rgb(theme.muted))
                        .child("Loading local projects..."),
                )
            })
            .when(!palette.projects.errors.is_empty(), |panel| {
                panel.child(
                    div()
                        .id("palette-project-errors")
                        .max_h(px(72.))
                        .overflow_y_scroll()
                        .flex_none()
                        .px(px(16.))
                        .py(px(8.))
                        .text_color(rgb(theme.muted))
                        .child(
                            palette
                                .projects
                                .errors
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join("\n"),
                        ),
                )
            })
            .when_some(palette.error.clone(), |panel, error| {
                panel.child(
                    div()
                        .id("palette-error")
                        .max_h(px(90.))
                        .overflow_y_scroll()
                        .flex_none()
                        .p(px(12.))
                        .text_color(rgb(theme.foreground))
                        .bg(rgb(theme.active))
                        .child(error),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(palette.filtered.is_empty(), |body| {
                        body.child(
                            div()
                                .flex_1()
                                .p(px(16.))
                                .text_color(rgb(theme.muted))
                                .when(!no_roots, |empty| {
                                    empty.child("No matching results. Try a shorter search.")
                                })
                                .when(no_roots, |empty| {
                                    empty
                                        .debug_selector(|| "palette-no-project-roots".into())
                                        .child(NO_PROJECT_ROOTS)
                                }),
                        )
                    })
                    .when(!palette.filtered.is_empty(), |body| {
                        body.child(
                            uniform_list(
                                "palette-results",
                                palette.filtered.len(),
                                cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                                    let Some(palette) = &this.menu.palette else {
                                        return Vec::new();
                                    };
                                    range
                                        .map(|index| this.palette_row(palette, index, cx))
                                        .collect()
                                }),
                            )
                            .track_scroll(&palette.scroll)
                            .flex_1()
                            .min_w_0(),
                        )
                    })
                    .when(previews, |body| {
                        body.child(self.render_palette_preview(palette))
                    }),
            )
            .child(
                div()
                    .debug_selector(|| "palette-status".into())
                    .flex_none()
                    .px(px(16.))
                    .py(px(10.))
                    .border_t_1()
                    .border_color(rgb(theme.active))
                    .text_color(rgb(theme.muted))
                    .child(if palette.busy() {
                        "Opening local project... Esc to dismiss."
                    } else if palette.scope == Scope::OpenProject {
                        OPEN_PROJECT_HINTS
                    } else if palette.filter == Filter::Navigation {
                        GO_TO_HINTS
                    } else {
                        HINTS
                    }),
            )
    }
}
