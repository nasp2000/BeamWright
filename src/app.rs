use egui::Ui;

#[derive(Default)]
pub struct BeamWrightApp {}

impl eframe::App for BeamWrightApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("menu_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |_ui| {});
                ui.menu_button("Edit", |_ui| {});
                ui.menu_button("Draw", |_ui| {});
                ui.menu_button("Arrange", |_ui| {});
                ui.menu_button("Operations", |_ui| {});
                ui.menu_button("Materials and profiles", |_ui| {});
                ui.menu_button("View", |_ui| {});
            });
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("BeamWright — skeleton");
        });
    }
}
