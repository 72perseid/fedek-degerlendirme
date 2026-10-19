use egui::{Color32, RichText};

use crate::app::{bold, App, Dialog, FetchTarget, Screen, Step};
use crate::ui::{card, hint, primary_button, secondary_button};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::Panel::top("home-top").show(ui, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading("FEDEK Ders Değerlendirme");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                zoom_buttons(app, ui);
                if secondary_button(ui, "Program Çıktıları (PÇ)").clicked() {
                    app.screen = Screen::Programs;
                }
            });
        });
        ui.add_space(6.0);
    });

    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(8.0);
            hint(ui, "Bir dersin dönem sonu ÖÇ-PÇ değerlendirmesini hazırlamak için yeni ders açın veya eski Excel dosyanızı içe aktarın. Yaptığınız her değişiklik kendiliğinden kaydedilir.");
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if primary_button(ui, "＋  Yeni Ders").clicked() {
                    if app.programs.len() == 1 {
                        let p = app.programs[0].clone();
                        app.new_course(p);
                    } else {
                        app.dialog = Some(Dialog::ChooseProgram);
                    }
                }
                if secondary_button(ui, "Bologna Bağlantısından Yeni Ders")
                    .on_hover_text("Bilgi paketindeki ders sayfasının bağlantısını yapıştırın; ders adı, ÖÇ'ler, PÇ'ler ve ilişki matrisi otomatik alınır.")
                    .clicked()
                {
                    app.dialog = Some(Dialog::BolognaUrl { text: String::new(), target: FetchTarget::NewCourse });
                }
                if secondary_button(ui, "Excel Dosyası İçe Aktar").clicked() {
                    app.import_excel();
                }
            });
            if let Some((msg, is_err)) = app.status_text() {
                let color = if is_err { crate::app::WARN_RED } else { Color32::DARK_GREEN };
                ui.label(RichText::new(msg).color(color).size(14.0));
            }
            ui.add_space(16.0);
            ui.label(bold("Dersleriniz").size(19.0));
            ui.add_space(4.0);

            if app.courses.is_empty() {
                ui.label(RichText::new("Henüz kayıtlı ders yok.").color(Color32::GRAY));
            }
            let mut action: Option<(String, &'static str)> = None;
            for cf in &app.courses {
                let c = &cf.course;
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            let title = if c.code_and_name.trim().is_empty() { "(adsız ders)".to_string() } else { c.code_and_name.clone() };
                            ui.label(bold(title).size(18.0));
                            let mut meta = Vec::new();
                            if !c.term.is_empty() { meta.push(c.term.clone()); }
                            if !c.instructor.is_empty() { meta.push(c.instructor.clone()); }
                            meta.push(format!("{} değerlendirme", c.assessments.len()));
                            meta.push(format!("son değişiklik {}", cf.updated_at));
                            ui.label(RichText::new(meta.join("  ·  ")).color(Color32::DARK_GRAY).size(14.0));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(RichText::new("Sil").color(crate::app::WARN_RED)).clicked() {
                                action = Some((cf.id.clone(), "delete"));
                            }
                            if ui.button("Yeni Döneme Kopyala").on_hover_text("Aynı ÖÇ ve PÇ eşleştirmesiyle, öğrenci ve puanları boş yeni bir dönem dosyası açar.").clicked() {
                                action = Some((cf.id.clone(), "copy"));
                            }
                            if primary_button(ui, "Aç").clicked() {
                                action = Some((cf.id.clone(), "open"));
                            }
                        });
                    });
                });
                ui.add_space(6.0);
            }
            if let Some((id, what)) = action {
                if let Some(cf) = app.courses.iter().find(|c| c.id == id).cloned() {
                    match what {
                        "open" => app.open_course(cf, Step::Info),
                        "copy" => app.copy_to_new_term(&cf),
                        _ => {
                            app.dialog = Some(Dialog::ConfirmDeleteCourse {
                                id,
                                title: format!("{} {}", cf.course.code_and_name, cf.course.term),
                            })
                        }
                    }
                }
            }
            ui.add_space(20.0);
            ui.label(RichText::new(format!("Veriler şu klasörde saklanır: {}", app.storage.root().display())).size(12.0).color(Color32::GRAY));
        });
    });
}

pub fn zoom_buttons(app: &mut App, ui: &mut egui::Ui) {
    let z = app.settings.zoom;
    if ui.add(egui::Button::new(RichText::new("A＋").size(16.0))).on_hover_text("Yazıları büyüt").clicked() {
        app.set_zoom(ui.ctx(), z + 0.1);
    }
    if ui.add(egui::Button::new(RichText::new("A－").size(16.0))).on_hover_text("Yazıları küçült").clicked() {
        app.set_zoom(ui.ctx(), z - 0.1);
    }
}
