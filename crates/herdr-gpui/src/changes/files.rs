//! Drawing a checkout's changed files: grouped into code, tests, docs and
//! the generated and binary files folded into one line, each with a bar
//! for how much of the checkout's change it is. A row opens its peek, the
//! diff under it; its review icon opens the file in a review tab.

use super::{
    Shown,
    noise::{self, Group},
    peek::{PEEK_LINES, State},
};
use crate::{
    HerdrWindow,
    fonts::StyledFont,
    review::{LineKind, Peeked, line_look, styled_code},
};
use gpui::{prelude::*, *};
use herdr_vcs::changes::{FileChange, Kind};

/// The widest the size bar draws, for the checkout's largest file.
const BAR: f32 = 36.;

impl HerdrWindow {
    /// The rows for `shown`'s changed files, in their groups.
    pub(super) fn changes_files(
        &self,
        id: &str,
        shown: &Shown,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let checkout = &shown.checkout;
        let groups = noise::grouped(&checkout.files, &self.config.changes.noise);
        let largest = checkout
            .files
            .iter()
            .map(weight)
            .max()
            .unwrap_or_default()
            .max(1);
        let labelled = groups.len() > 1;
        let mut rows = Vec::new();
        for (group, files) in &groups {
            if *group == Group::Noise {
                let open = self.changes.peeks.noise_shown.contains(&checkout.path);
                rows.push(self.changes_noise_row(id, &checkout.path, files, open, cx));
                if !open {
                    continue;
                }
            } else if labelled {
                rows.push(self.changes_group_label(group.label()));
            }
            for file in files {
                rows.push(self.changes_file(id, shown, file, largest, cx));
                let key = (checkout.path.clone(), file.path.clone());
                if let Some(peek) = self.changes.peeks.get(&key) {
                    rows.push(self.changes_peek(id, shown, file, &peek.state, cx));
                }
            }
        }
        rows
    }

    fn changes_group_label(&self, label: &'static str) -> AnyElement {
        div()
            .px_3()
            .pl(px(32.))
            .pt(px(4.))
            .pb(px(1.))
            .text_size(px(self.config.ui.size - 2.))
            .text_color(rgb(self.theme.muted))
            .child(label)
            .into_any_element()
    }

    fn changes_noise_row(
        &self,
        id: &str,
        checkout: &str,
        files: &[&FileChange],
        open: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = &self.theme;
        let count = files.len();
        let noun = if count == 1 { "file" } else { "files" };
        let path = checkout.to_owned();
        div()
            .id(SharedString::from(format!("changes-noise-{id}")))
            .debug_selector(|| "changes-noise".into())
            .px_3()
            .pl(px(18.))
            .pt(px(4.))
            .pb(px(1.))
            .flex()
            .items_center()
            .gap_1()
            .cursor_pointer()
            .text_size(px(self.config.ui.size - 2.))
            .text_color(rgb(theme.muted))
            .hover(|style| style.text_color(rgb(theme.foreground)))
            .child(chevron(open, theme.muted))
            .child(format!(
                "{} · {count} {noun} ({})",
                Group::Noise.label(),
                noise::tally(files)
            ))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_changes_noise(path.clone(), cx);
            }))
            .into_any_element()
    }

    fn changes_file(
        &self,
        id: &str,
        shown: &Shown,
        file: &FileChange,
        largest: u64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = &self.theme;
        let small = px(self.config.ui.size - 1.);
        let (green, red) = (theme.ink(theme.palette[2]), theme.ink(theme.palette[1]));
        let color = match file.kind {
            Kind::Added => green,
            Kind::Conflicted | Kind::Deleted => red,
            Kind::Modified => theme.ink(theme.palette[3]),
            Kind::Renamed => theme.ink(theme.palette[4]),
            Kind::Untracked => theme.muted,
        };
        let counts = match (file.additions, file.deletions) {
            (Some(added), Some(removed)) => Some(format!("+{added} −{removed}")),
            _ if file.kind == Kind::Untracked => Some("new".into()),
            _ => None,
        };
        let key = (shown.checkout.path.clone(), file.path.clone());
        let open = self.changes.peeks.get(&key).is_some();
        // A folder Git lists whole, untracked, has no diff of its own.
        let peekable = !file.path.ends_with('/');
        let trimmed = file.path.trim_end_matches('/');
        let (folder, name) = match trimmed.rsplit_once('/') {
            Some((folder, name)) => (format!("{folder}/"), name.to_owned()),
            None => (String::new(), trimmed.to_owned()),
        };
        let name = if file.path.ends_with('/') {
            format!("{name}/")
        } else {
            name
        };
        let bar = (weight(file) > 0).then(|| {
            let width = (BAR * weight(file) as f32 / largest as f32).max(2.);
            let added = file.additions.unwrap_or_default() as f32;
            let share = added / weight(file) as f32;
            div()
                .flex_none()
                .w(px(BAR))
                .flex()
                .child(div().h(px(4.)).w(px(width * share)).bg(rgb(green)))
                .child(div().h(px(4.)).w(px(width * (1. - share))).bg(rgb(red)))
        });
        let review = {
            let shown = shown.clone();
            let path = file.path.clone();
            div()
                .id(SharedString::from(format!(
                    "changes-review-{id}-{}",
                    file.path
                )))
                .debug_selector(|| "changes-file-review".into())
                .flex_none()
                .p(px(2.))
                .rounded_sm()
                .cursor_pointer()
                .hover(|style| style.bg(rgb(theme.active)))
                .child(
                    svg()
                        .path("icons/diff-unified.svg")
                        .size(px(12.))
                        .text_color(rgb(theme.subtext())),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.review_changes_file(&shown, &path, window, cx);
                }))
        };
        let hover = theme.active;
        div()
            .id(SharedString::from(format!(
                "changes-file-{id}-{}",
                file.path
            )))
            .debug_selector(|| "changes-file".into())
            .px_3()
            .pl(px(18.))
            .py(px(1.))
            .flex()
            .items_center()
            .gap_2()
            .text_size(small)
            .when(peekable, |el| {
                el.cursor_pointer().hover(move |style| style.bg(rgb(hover)))
            })
            .child(
                div()
                    .flex_none()
                    .w(px(10.))
                    .when(peekable, |el| el.child(chevron(open, theme.muted))),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(color))
                    .child(file.kind.mark()),
            )
            .child(div().flex_1().min_w_0().truncate().child(
                StyledText::new(format!("{folder}{name}")).with_highlights([(
                    0..folder.len(),
                    HighlightStyle {
                        color: Some(rgb(theme.muted).into()),
                        ..HighlightStyle::default()
                    },
                )]),
            ))
            .children(bar)
            .children(
                counts.map(|counts| div().flex_none().text_color(rgb(theme.muted)).child(counts)),
            )
            .child(review)
            .when(peekable, |el| {
                el.on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_changes_peek(key.clone(), cx);
                }))
            })
            .into_any_element()
    }

    fn changes_peek(
        &self,
        id: &str,
        shown: &Shown,
        file: &FileChange,
        state: &State,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = &self.theme;
        let small = px(self.config.ui.size - 1.);
        let note = |text: String| {
            div()
                .py(px(4.))
                .text_size(small)
                .text_color(rgb(theme.muted))
                .child(text)
        };
        let body = match state {
            State::Reading => note("Reading…".into()),
            State::Failed(why) => note(why.clone()),
            State::Read(Peeked::Binary) => note("Binary file".into()),
            State::Read(Peeked::Large) => note("Too large to peek; open it in a review".into()),
            State::Read(Peeked::Gone) => note("No longer changed".into()),
            State::Read(Peeked::Failed(why)) => note(why.clone()),
            State::Read(Peeked::Lines {
                lines,
                spans,
                words,
            }) => {
                let mut code = div()
                    .flex()
                    .flex_col()
                    .text_font(&self.config.terminal)
                    .text_size(px(self.config.terminal.size - 1.));
                for (index, line) in lines.iter().take(PEEK_LINES).enumerate() {
                    let (sign, tint, word_tint, text) = line_look(theme, line.kind);
                    let number = match line.kind {
                        LineKind::Removed => line.old,
                        _ => line.new,
                    };
                    let row = if line.kind == LineKind::Hunk {
                        div()
                            .px_1()
                            .text_color(rgb(theme.muted))
                            .truncate()
                            .child(lines.text_of(line).to_owned())
                    } else {
                        div()
                            .flex()
                            .when_some(tint, |el, tint| el.bg(tint))
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(32.))
                                    .pr_1()
                                    .flex()
                                    .justify_end()
                                    .text_color(rgb(theme.muted))
                                    .children(number.map(|number| number.to_string())),
                            )
                            .child(div().flex_none().w(px(12.)).child(sign))
                            .child(div().flex_1().min_w_0().text_color(rgb(text)).child(
                                styled_code(
                                    theme,
                                    lines.text_of(line),
                                    spans.get(index).map_or(&[], Vec::as_slice),
                                    words.get(index).map_or(&[], Vec::as_slice),
                                    word_tint,
                                    None,
                                ),
                            ))
                    };
                    code = code.child(row);
                }
                let more = lines.len().saturating_sub(PEEK_LINES);
                if lines.is_empty() {
                    code = code.child(note("No lines changed".into()));
                } else if more > 0 {
                    code = code.child(note(format!("{more} more lines in the review")));
                }
                code
            }
        };
        let shown = shown.clone();
        let path = file.path.clone();
        div()
            .debug_selector(|| "changes-peek".into())
            .ml(px(30.))
            .mr_3()
            .my(px(2.))
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(rgb(theme.active))
            .bg(rgb(theme.background))
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(body)
            .child(
                div()
                    .id(SharedString::from(format!(
                        "changes-peek-review-{id}-{path}"
                    )))
                    .pt_1()
                    .text_size(small)
                    .text_color(rgb(theme.ink(theme.palette[4])))
                    .cursor_pointer()
                    .hover(|style| style.underline())
                    .child("Open in review")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.review_changes_file(&shown, &path, window, cx);
                    })),
            )
            .into_any_element()
    }
}

/// Changed lines, for the size bar.
fn weight(file: &FileChange) -> u64 {
    file.additions.unwrap_or_default() + file.deletions.unwrap_or_default()
}

fn chevron(open: bool, color: u32) -> Svg {
    svg()
        .path(if open {
            "icons/chevron-down.svg"
        } else {
            "icons/chevron-right.svg"
        })
        .size(px(10.))
        .flex_none()
        .text_color(rgb(color))
}
