//! Shared geometry for the shell's round icon controls. Optical corrections live
//! inside each SVG; focus, hover and pointer targets stay on the unchanged slot.
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::{ElementId, SharedString, Styled, px};

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
