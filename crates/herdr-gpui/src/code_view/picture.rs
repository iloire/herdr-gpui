//! A code tab on a picture: the file decoded off the UI thread, as a
//! backdrop is (`backdrop::decode`), and drawn whole in the tab, never
//! larger than it is, under a header with its size. Its editor is never
//! drawn, so it takes no keys, and never reads binary text, so it cannot
//! save over the file.

use super::CodeView;
use crate::{
    HerdrWindow,
    backdrop::{self, Error},
    browser::{Location, Store, Tab},
};
use gpui::{prelude::*, *};
use std::{path::Path, sync::Arc};

/// The extensions of the formats the app decodes.
const EXTENSIONS: [&str; 5] = ["gif", "jpeg", "jpg", "png", "webp"];

/// Whether `path` names a picture, which its tab draws rather than edits.
pub(crate) fn is_picture(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

/// The picture `tab` shows, if it is a code tab on one.
pub(super) fn of(tab: &Tab) -> Option<&Path> {
    match &tab.location {
        Some(Location::Code { file }) => {
            Some(Path::new(&file.path)).filter(|path| is_picture(path))
        }
        _ => None,
    }
}

/// A picture as last read.
pub(crate) struct Picture {
    pub(super) texture: Arc<RenderImage>,
    /// Its own width and height, before scaling to fit a texture.
    pub(super) size: (u32, u32),
}

impl Picture {
    /// Reads and decodes the picture at `path`. Blocking.
    pub(super) fn read(path: &Path) -> Result<Self, Error> {
        let (texture, size) = backdrop::decode_sized(path)?;
        Ok(Self {
            texture: Arc::new(texture),
            size,
        })
    }
}

/// Why a picture could not be shown, worded for its tab rather than for a
/// backdrop.
fn reason(error: &Error) -> String {
    match error {
        Error::Read { source, .. } => format!("Could not read this file: {source}"),
        Error::NotAFile { .. } => "This is not a regular file".into(),
        Error::TooLarge { .. } => "This picture is larger than 64 MiB".into(),
        Error::Decode { source, .. } => format!("Could not decode this picture: {source}"),
        other => other.to_string(),
    }
}

impl HerdrWindow {
    /// Frees the textures of pictures replaced since, and of tabs closed,
    /// before their views go. Runs every window tick, where the window is
    /// at hand.
    pub(crate) fn release_code_pictures(&mut self, window: &mut Window, cx: &App) {
        let store = cx.try_global::<Store>();
        for (id, view) in &mut self.code_views {
            for texture in view.released.drain(..) {
                let _ = window.drop_image(texture);
            }
            if store.is_none_or(|store| store.get(*id).is_none())
                && let Some(Ok(picture)) = view.picture.take()
            {
                let _ = window.drop_image(picture.texture);
            }
        }
    }

    /// The header's word on a picture: its size, and opening it in the
    /// system's viewer.
    pub(super) fn picture_header(&self, view: &CodeView, path: &Path) -> Vec<AnyElement> {
        let theme = &self.theme;
        let mut items = Vec::new();
        if let Some(Ok(Picture {
            size: (width, height),
            ..
        })) = &view.picture
        {
            items.push(
                div()
                    .debug_selector(|| "code-picture-size".into())
                    .flex_none()
                    .text_color(rgb(theme.muted))
                    .child(format!("{width} \u{00d7} {height}"))
                    .into_any_element(),
            );
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let path = path.to_path_buf();
            items.push(
                div()
                    .id("code-picture-open")
                    .debug_selector(|| "code-picture-open".into())
                    .flex_none()
                    .px_2()
                    .py_1()
                    .rounded(px(crate::config::corners::CONTROL))
                    .cursor_pointer()
                    .hover(|button| button.bg(rgb(theme.active)))
                    .child("Open")
                    .on_click(move |_, _, cx| crate::folder::open_path(false, path.clone(), cx))
                    .into_any_element(),
            );
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let _ = path;
        items
    }

    /// The picture, whole and centred, or why it is not shown.
    pub(super) fn picture_body(&self, view: &CodeView) -> AnyElement {
        let body = div()
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .p_4();
        match &view.picture {
            None => body.into_any_element(),
            Some(Ok(picture)) => body
                .child(
                    img(picture.texture.clone())
                        .debug_selector(|| "code-picture".into())
                        .size_full()
                        .object_fit(ObjectFit::ScaleDown),
                )
                .into_any_element(),
            Some(Err(error)) => body
                .child(
                    div()
                        .debug_selector(|| "code-picture-failed".into())
                        .text_color(crate::menu::danger(&self.theme))
                        .child(reason(error)),
                )
                .into_any_element(),
        }
    }
}
