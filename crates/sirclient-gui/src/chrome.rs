//! Native window chrome: make the title bar blend into the app's dark
//! background instead of showing the platform's default (often light) one.
//!
//! - macOS: the title bar is transparent and the content runs underneath it,
//!   so the traffic lights float over the sidebar like in native apps.
//! - Windows 11: dark mode plus a DWM caption colour equal to the background.
//! - Linux: ask for the dark theme variant; GTK-style decorations (X11 with
//!   `_GTK_THEME_VARIANT`, Wayland client-side frames) follow it. Tiling
//!   compositors without title bars are unaffected.

/// Height of the area at the top of the window that the (transparent) macOS
/// title bar covers. Content must leave this free so the traffic lights
/// don't overlap it.
#[cfg(target_os = "macos")]
pub const TITLEBAR_H: f32 = 28.0;
#[cfg(not(target_os = "macos"))]
pub const TITLEBAR_H: f32 = 0.0;

pub fn viewport(b: egui::ViewportBuilder) -> egui::ViewportBuilder {
    #[cfg(target_os = "macos")]
    let b = b
        .with_fullsize_content_view(true)
        .with_titlebar_shown(false)
        .with_title_shown(false);
    b
}

/// Called once the native window exists.
pub fn init(cc: &eframe::CreationContext<'_>) {
    // Dark title bar / traffic lights / native dialogs on every platform.
    cc.egui_ctx
        .send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));
    #[cfg(windows)]
    if let Some(w) = cc.winit_window() {
        use crate::theme::{BG, BORDER};
        use winit::platform::windows::{Color, WindowExtWindows};
        let c = |c: egui::Color32| Color::from_rgb(c.r(), c.g(), c.b());
        // Windows 11 22000+; older versions ignore these.
        w.set_title_background_color(Some(c(BG)));
        w.set_border_color(Some(c(BORDER)));
    }
}

/// macOS: let the empty strip under the transparent title bar move the window
/// (and double-click zoom it), since our content now covers that area.
pub fn title_bar_drag(ui: &mut egui::Ui) {
    if TITLEBAR_H <= 0.0 {
        return;
    }
    let screen = ui.ctx().content_rect();
    let rect = egui::Rect::from_min_size(screen.min, egui::vec2(screen.width(), TITLEBAR_H));
    let r = ui.interact(
        rect,
        ui.scope_id().with("titlebar-drag"),
        egui::Sense::click_and_drag(),
    );
    if r.double_clicked() {
        let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
    } else if r.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
}
