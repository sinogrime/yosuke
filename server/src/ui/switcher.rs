use crate::ui::view::{View, ViewPage};
use egui::{Align, Context, Frame, Layout, Margin, TopBottomPanel};

pub fn render(view: &mut View, ctx: &Context) {
    // create a panel covering the top of the viewport
    TopBottomPanel::top("switcher").show(ctx, |ui| { // unique ID of 'switcher'
        ui.horizontal(|ui| { // put these buttons on the same row
            Frame::new() // wrap within frame so we can add padding
                .inner_margin(Margin {
                    left: 0,
                    right: 0,
                    top: 6,
                    bottom: 6,
                })
                .show(ui, |ui| {
                    if ui.button("🖧  Sessions").clicked() { // nav to sessions page
                        view.state.page = ViewPage::Sessions;
                    }
                    if ui.button("🛠  Builder").clicked() { // nav to builder page
                        view.state.page = ViewPage::Builder;
                    }
                });

            #[cfg(debug_assertions)]
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label("⚠  Debug build");
                // if server was built in debug mode, show this warning on the right side of the switcher
            });
        });
    });
}
