//! The few DOM operations Dioxus doesn't expose, run as JavaScript with `document::eval`.
//!
//! `document::eval` works with the web renderer and in the desktop WebView alike.

use dioxus::prelude::*;

/// Opens the `<dialog>` with `id` as a modal, unless it's already open.
///
/// The browser makes the rest of the page inert and moves focus into the dialog. We remember the
/// focused element so [`close_modal`] can return focus to it, and keep Tab cycling inside the
/// dialog.
pub fn show_modal(id: &str) {
    // A modal dialog lets Tab leave the page for the browser's own interface. A desktop WebView
    // has no such interface, so we wrap around between the first and last controls instead.
    document::eval(&format!(
        r#"
        const dialog = document.getElementById({id:?});
        if (!dialog.wrapsFocus) {{
            dialog.wrapsFocus = true;
            dialog.addEventListener("keydown", (event) => {{
                if (event.key !== "Tab") {{
                    return;
                }}
                const controls = dialog.querySelectorAll(
                    "button:not([disabled]), [href], input:not([disabled]), [tabindex]:not([tabindex='-1'])"
                );
                const first = controls[0];
                const last = controls[controls.length - 1];
                if (event.shiftKey && document.activeElement === first) {{
                    event.preventDefault();
                    last.focus();
                }} else if (!event.shiftKey && document.activeElement === last) {{
                    event.preventDefault();
                    first.focus();
                }}
            }});
        }}
        if (!dialog.open) {{
            dialog.returnFocus = document.activeElement;
            dialog.showModal();
        }}
        "#
    ));
}

/// Closes the `<dialog>` with `id`, and returns focus to the element focused when it opened.
///
/// When that element is gone, or nothing was focused, focus moves to the element with
/// `fallback_id` instead.
pub fn close_modal(id: &str, fallback_id: &str) {
    // We restore focus whenever the dialog remembers an element, not only when it's still open,
    // because pressing Escape closes it before we get here.
    document::eval(&format!(
        r#"
        const dialog = document.getElementById({id:?});
        if (dialog.open) {{
            dialog.close();
        }}
        const target = dialog.returnFocus;
        dialog.returnFocus = null;
        if (target) {{
            const restored = target.isConnected && target !== document.body
                ? target
                : document.getElementById({fallback_id:?});
            restored?.focus();
        }}
        "#
    ));
}

/// Focuses the element with `id`, if there is one.
pub fn focus(id: &str) {
    document::eval(&format!("document.getElementById({id:?})?.focus();"));
}

/// Focuses the first element of `ids` that exists.
pub fn focus_first(ids: &[&str]) {
    document::eval(&format!(
        r#"
        const element = {ids:?}
            .map((id) => document.getElementById(id))
            .find((element) => element !== null);
        element?.focus();
        "#
    ));
}

/// Scrolls the element with `id` into view inside its scrolling container, if it isn't already.
pub fn scroll_into_view(id: &str) {
    document::eval(&format!(
        r#"document.getElementById({id:?})?.scrollIntoView({{ block: "nearest" }});"#
    ));
}

/// Measures the height of each child of the element with `id`, in CSS pixels.
///
/// Measure elements outside any CSS transform, which would scale the result. Returns an empty
/// list when the element doesn't exist or the page doesn't answer.
pub async fn child_heights(id: &str) -> Vec<f64> {
    let script = format!(
        r#"
        const parent = document.getElementById({id:?});
        if (!parent) {{
            return [];
        }}
        return [...parent.children].map((child) => child.getBoundingClientRect().height);
        "#
    );

    let Ok(heights) = document::eval(&script).await else {
        return Vec::new();
    };

    serde_json::from_value(heights).unwrap_or_default()
}

/// Sends pointer `pointer_id`'s events to the element with `id` until the pointer is released,
/// even once it leaves the element.
pub fn capture_pointer(id: &str, pointer_id: i32) {
    // Capturing fails when the pointer was released meanwhile, which needs no handling.
    document::eval(&format!(
        r#"
        try {{
            document.getElementById({id:?})?.setPointerCapture({pointer_id});
        }} catch {{}}
        "#
    ));
}

/// Focuses the element with `id` once it's in the document, waiting a few frames for it.
///
/// For elements a state change is about to render.
pub fn focus_soon(id: &str) {
    document::eval(&format!(
        r#"
        let frames = 0;
        const attempt = () => {{
            const element = document.getElementById({id:?});
            if (element) {{
                element.focus();
            }} else if (frames++ < 10) {{
                requestAnimationFrame(attempt);
            }}
        }};
        requestAnimationFrame(attempt);
        "#
    ));
}

/// Waits `milliseconds`, with the page's timer so it works with every renderer.
pub async fn sleep(milliseconds: u32) {
    let script = format!(
        "await new Promise((resolve) => setTimeout(resolve, {milliseconds})); return true;"
    );

    // Not answering only makes the wait shorter.
    let _ = document::eval(&script).await;
}

/// Scrolls the element with `id` down by `dy` pixels, up when negative.
pub fn scroll_by(id: &str, dy: f64) {
    document::eval(&format!(
        "document.getElementById({id:?})?.scrollBy({{ top: {dy} }});"
    ));
}

/// Scrolls the element with `id` to `top` after the next frame, once new content rendered.
pub fn scroll_top_soon(id: &str, top: f64) {
    document::eval(&format!(
        r#"
        requestAnimationFrame(() => {{
            const element = document.getElementById({id:?});
            if (element) {{
                element.scrollTop = {top};
            }}
        }});
        "#
    ));
}

/// Focuses the first element of `ids` that exists, when the focused element was removed.
///
/// Waits for the next frame, so the state change that removed it has rendered. Desktop only, for
/// auto-reload.
#[cfg(feature = "desktop")]
pub fn focus_if_lost(ids: &[&str]) {
    document::eval(&format!(
        r#"
        requestAnimationFrame(() => {{
            const focused = document.activeElement;
            if (focused && focused !== document.body && focused.isConnected) {{
                return;
            }}
            const element = {ids:?}
                .map((id) => document.getElementById(id))
                .find((element) => element !== null);
            element?.focus();
        }});
        "#
    ));
}
