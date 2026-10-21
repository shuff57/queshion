//! Test support: drive real controls in-process with egui_kittest (simulated typing, paste, clicks
//! and drags, found by their accessibility label). The app's own fonts are installed first,
//! because the controls use named families (`vs-sans-bold`) that a bare egui context lacks.
use egui_kittest::Harness;

/// A harness around `app` with the app's theme and fonts loaded. egui applies new fonts on the
/// next pass, so the first frame (which the harness runs on construction) only installs them and
/// draws nothing; the harness then runs on to a settled frame.
pub fn harness<'a, S>(mut app: impl FnMut(&mut egui::Ui, &mut S) + 'a, state: S) -> Harness<'a, S> {
    let mut first = true;
    // short frames, so two clicks a few frames apart still count as a double-click
    Harness::builder().with_step_dt(0.01).build_ui_state(
        move |ui, s: &mut S| {
            if std::mem::take(&mut first) {
                crate::theme::setup(ui.ctx());
                ui.ctx().request_repaint();
                return;
            }
            app(ui, s)
        },
        state,
    )
}
