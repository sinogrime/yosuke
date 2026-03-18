use std::collections::HashMap;

use eframe::{App, Frame};
use egui::{CentralPanel, Context};
use egui_notify::Toasts;

use crate::{
    types::mouthpieces::UiMouthpiece,
    ui::{client::ClientView, pages, switcher, updates},
};

pub struct ViewBuilderSettings {
    pub address: String,
    pub port: String,
}
pub enum ViewPage { // custom enumerator for UI pages
    Sessions, // you can either be on this page
    Builder, // or this page
}
pub struct ViewLogs {
    pub server: Vec<String>,
    pub builder: Vec<String>,
}
pub struct ViewState {
    pub notifications: Toasts, // 'egui_notify' struct
    pub clients: HashMap<String, ClientView>, // hash table!
    pub page: ViewPage, // enum mentioend above
    pub logs: ViewLogs, // defined above
    pub builder: ViewBuilderSettings, // defined above
    pub listening: bool, // simple boolean for if server should be listening (network)
}
impl ViewState {
    pub fn new() -> Self { // constructor for ViewState
        Self { // initialises default values for all fields
            notifications: Toasts::default(), 
            clients: HashMap::new(),
            page: ViewPage::Sessions,
            logs: ViewLogs {
                server: Vec::new(),
                builder: Vec::new(),
            },
            builder: ViewBuilderSettings {
                address: String::from("127.0.0.1"),
                port: String::from("5317"),
            },
            listening: false,
        }
    }
}

pub struct View {
    pub state: ViewState, // defined above

    // UI <--> Server
    // UI <--> Manager
    // UI <--> Builder
    pub mouthpiece: UiMouthpiece,
}
impl View {
    pub fn new(mouthpiece: UiMouthpiece) -> Self { // constructor
        Self {
            state: ViewState::new(),
            mouthpiece: mouthpiece, // given as argument
        }
    }
}

// we must implement the egui 'App' trait for our 'View' struct
//           so we can use it as the main application in eframe
impl App for View {
    // 'ctx' is given to us by egui to let us handle UI context.
    // '_frame' is also given to us by egui, but we don't need it so we prefix with an underscore.
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {

        // if we have received a new notification, paint it
        self.state.notifications.show(ctx);

        updates::server(self, ctx); // listen for updates from the server mouthpiece
        updates::manager(self, ctx); // listen for updates from the manager mouthpiece 

        switcher::render(self, ctx); // render the page switcher (top bar with buttons to switch between pages)

        CentralPanel::default().show(ctx, |ui| match &self.state.page {
            ViewPage::Sessions => {
                pages::sessions::render(self, ui); // display the home page of sessions if chosen
            }
            ViewPage::Builder => {
                pages::builder::render(self, ui); // else display the builder page
            }
        });

        ctx.request_repaint(); // Repaint every frame so UI does not lock up when mouse is not moving
    }
}