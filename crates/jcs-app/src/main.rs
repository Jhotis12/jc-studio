//! JC Studio desktop application.
//!
//! First slice: a real installable window with the layout the product needs. The daemon
//! client, the task board and the embedded terminal arrive in the next steps; keeping this
//! commit runnable means the packaging pipeline can be exercised from day one.

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("JC Studio"),
        ..Default::default()
    };

    eframe::run_native(
        "JC Studio",
        options,
        Box::new(|_context| Ok(Box::new(Studio::default()))),
    )
}

#[derive(Default)]
struct Studio {
    objective: String,
}

impl eframe::App for Studio {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header").show(context, |ui| {
            ui.horizontal(|ui| {
                ui.heading("JC Studio");
                ui.separator();
                ui.label("daemon: not connected yet");
            });
        });

        egui::SidePanel::left("projects")
            .resizable(true)
            .default_width(240.0)
            .show(context, |ui| {
                ui.heading("Projects");
                ui.separator();
                ui.label("No projects registered.");
            });

        egui::SidePanel::right("details")
            .resizable(true)
            .default_width(360.0)
            .show(context, |ui| {
                ui.heading("Details");
                ui.separator();
                ui.label("Select a goal to see its tasks, gates and runs.");
            });

        egui::CentralPanel::default().show(context, |ui| {
            ui.heading("Goals");
            ui.separator();
            ui.label("The task board will render here.");
            ui.add_space(16.0);
            ui.label("New objective:");
            ui.add(
                egui::TextEdit::multiline(&mut self.objective)
                    .desired_rows(3)
                    .hint_text("What should the agents accomplish?"),
            );
            ui.horizontal(|ui| {
                let _ = ui.button("Queue objective");
                let _ = ui.button("Render policy");
            });
        });

        egui::TopBottomPanel::bottom("terminals").show(context, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Terminals");
                ui.separator();
                ui.label("No terminal open.");
            });
            ui.allocate_space(egui::vec2(ui.available_width(), 160.0));
        });
    }
}
