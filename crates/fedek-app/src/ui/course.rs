//! The course editor shell: top bar, step list on the left, step content.

use egui::{Color32, RichText};
use fedek_core::{run_checks, Severity};

use crate::app::{bold, App, Step, ACCENT};
use crate::ui::{home::zoom_buttons, primary_button, secondary_button};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let (title, warnings) = match app.open.as_ref() {
        Some(open) => {
            let c = &open.file.course;
            let t = if c.code_and_name.trim().is_empty() { "Yeni ders".to_string() } else { c.code_and_name.clone() };
            let title = if c.term.trim().is_empty() { t } else { format!("{t}  ·  {}", c.term) };
            let warnings = run_checks(c).into_iter().filter(|c| c.severity == Severity::Warning).count();
            (title, warnings)
        }
        None => {
            app.screen = crate::app::Screen::Home;
            return;
        }
    };

    egui::Panel::top("course-top").show(ui, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if secondary_button(ui, "◀  Ana Sayfa").clicked() {
                app.go_home();
                return;
            }
            ui.add_space(8.0);
            ui.label(bold(title.clone()).size(18.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                zoom_buttons(app, ui);
                if primary_button(ui, "Excel'e Aktar").clicked() {
                    app.export_excel();
                }
                if primary_button(ui, "Rapor (PDF)").on_hover_text("Yazdırılabilir A4 rapor").clicked() {
                    app.export_pdf();
                }
                let (label, color) = app.save_state_label();
                ui.label(RichText::new(label).color(color));
                if let Some((msg, is_err)) = app.status_text() {
                    let color = if is_err { crate::app::WARN_RED } else { Color32::DARK_GREEN };
                    ui.label(RichText::new(msg).color(color).size(14.0));
                }
            });
        });
        ui.add_space(6.0);
    });
    if app.open.is_none() {
        return;
    }

    egui::Panel::left("steps")
        .resizable(false)
        .exact_size(250.0)
        .show(ui, |ui| {
            ui.add_space(10.0);
            ui.label(RichText::new("ADIMLAR").small().color(Color32::GRAY));
            ui.add_space(4.0);
            let current = app.open.as_ref().map(|o| o.step).unwrap();
            for (i, step) in Step::ALL.iter().enumerate() {
                let selected = *step == current;
                let mut label = format!("{}.  {}", i + 1, step.title());
                if *step == Step::Results && warnings > 0 {
                    label.push_str(&format!("   ⚠ {warnings}"));
                }
                let text = if selected { RichText::new(label).color(Color32::WHITE).size(16.0) } else { RichText::new(label).size(16.0) };
                let btn = egui::Button::new(text)
                    .fill(if selected { ACCENT } else { Color32::TRANSPARENT })
                    .min_size(egui::vec2(230.0, 44.0));
                if ui.add(btn).clicked() {
                    if let Some(o) = app.open.as_mut() {
                        o.step = *step;
                    }
                }
                ui.add_space(2.0);
            }
            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(RichText::new("Pembe alanlar doldurulacak alanlardır. Değişiklikler kendiliğinden kaydedilir.").size(13.0).color(Color32::GRAY));
        });

    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(6.0);
            let step = app.open.as_ref().map(|o| o.step).unwrap();
            match step {
                Step::Info => crate::ui::course_info::show(app, ui),
                Step::Outcomes => crate::ui::outcomes::show(app, ui),
                Step::Assessments => crate::ui::assessments::show(app, ui),
                Step::Results => crate::ui::results::show(app, ui),
            }
            ui.add_space(30.0);
        });
    });
}
