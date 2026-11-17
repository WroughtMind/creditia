//! Shared geometry for the shell's round icon controls. Optical corrections live
//! inside each SVG; focus, hover and pointer targets stay on the unchanged slot.
use gpui_kit::accesskit::Role;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Disableable, Selectable};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub fn icon_button(
    id: impl Into<ElementId>,
    glyph: &str,
    label: impl Into<SharedString>,
) -> Button {
    let label = label.into();
    Button::new(id)
        .ghost()
        .w(px(32.))
        .h(px(32.))
        .p_0()
        .rounded_full()
        .icon(crate::ui_icons::icon(glyph))
        .accessibility_label(label.clone())
        .tooltip(label)
}

fn row_content(label: SharedString) -> Div {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(14.))
                .line_height(px(20.))
                .truncate()
                .child(label),
        )
}

/// Full-row actions keep labels on one reading edge and reserve a fixed icon slot.
pub fn row_button(id: impl Into<ElementId>, label: impl Into<SharedString>, glyph: &str) -> Button {
    let label = label.into();
    Button::new(id)
        .ghost()
        .w_full()
        .h(px(40.))
        .px_2()
        .accessibility_label(label.clone())
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap_2()
                .child(crate::ui_icons::icon(glyph))
                .child(row_content(label)),
        )
}

pub fn value_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    cx: &App,
) -> Button {
    let label = label.into();
    let value = value.into();
    Button::new(id)
        .ghost()
        .w_full()
        .h(px(40.))
        .px_2()
        .accessibility_label(format!("{label}: {value}"))
        .child(
            row_content(label)
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(cx.theme().muted_foreground)
                        .child(value),
                )
                .child(crate::ui_icons::icon("updown")),
        )
}

/// A single focusable switch row: pointer and keyboard use the same handler,
/// with switch role/toggled metadata. The track and thumb are decorative children.
pub fn toggle_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
    enabled: bool,
    cx: &App,
) -> Button {
    let label = label.into();
    let track = div()
        .relative()
        .w(px(32.))
        .h(px(18.))
        .flex_shrink_0()
        .rounded_full()
        .bg(if checked {
            cx.theme().primary
        } else {
            cx.theme().muted_foreground.opacity(0.3)
        })
        .opacity(if enabled { 1. } else { 0.45 })
        .child(
            div()
                .absolute()
                .top(px(2.))
                .left(px(if checked { 16. } else { 2. }))
                .w(px(14.))
                .h(px(14.))
                .rounded_full()
                .bg(cx.theme().background),
        );
    Button::new(id)
        .ghost()
        .w_full()
        .h(px(40.))
        .px_2()
        .role(Role::Switch)
        .toggled(checked)
        .disabled(!enabled)
        .accessibility_label(label.clone())
        .child(row_content(label).child(track))
}

pub fn choice_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    chosen: bool,
    preview: Option<AnyElement>,
) -> Button {
    let label = label.into();
    Button::new(id)
        .ghost()
        .h(px(40.))
        .px_2()
        .selected(chosen)
        .toggled(chosen)
        .accessibility_label(label.clone())
        .tooltip(label.clone())
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap_2()
                .when_some(preview, |row, preview| {
                    row.child(
                        div()
                            .w(px(24.))
                            .h(px(24.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(preview),
                    )
                })
                .child(row_content(label))
                .child(crate::ui_icons::icon("check").opacity(if chosen { 1. } else { 0. })),
        )
}
