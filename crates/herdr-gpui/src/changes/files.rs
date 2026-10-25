//! Drawing a checkout's changed files: grouped into code, tests, docs and
//! the generated and binary files folded into one line, each with a bar
//! for how much of the checkout's change it is. A row shows the checkout's
//! uncommitted changes beside the list, at that file.

use super::{
    Shown,
    noise::{self, Group},
    pane::Pick,
};
use crate::HerdrWindow;
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
                let open = self.changes.noise_shown.contains(&checkout.path);
                rows.push(self.changes_noise_row(id, &checkout.path, files, open, cx));
                if !open {
                    continue;
                }
            } else if labelled {
                rows.push(self.changes_group_label(group.label()));
            }
            for file in files {
                rows.push(self.changes_file(id, shown, file, largest, cx));
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
        // A folder Git lists whole, untracked, has no diff of its own.
        let reviewable = !file.path.ends_with('/');
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
        let hover = theme.active;
        div()
            .id(SharedString::from(format!(
                "changes-file-{id}-{}",
                file.path
            )))
            .debug_selector(|| "changes-file".into())
            .px_3()
            .pl(px(32.))
            .py(px(1.))
            .flex()
            .items_center()
            .gap_2()
            .text_size(small)
            .when(reviewable, |el| {
                el.cursor_pointer().hover(move |style| style.bg(rgb(hover)))
            })
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
            .when(reviewable, |el| {
                let (pick, path) = (
                    Pick::Uncommitted(shown.checkout.path.clone()),
                    file.path.clone(),
                );
                el.on_click(cx.listener(move |this, _, window, cx| {
                    this.click_change(pick.clone(), Some(path.clone()), window, cx);
                }))
            })
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
