//! Modal dialogs: confirmations, errors, roster paste, programme choice.

use egui::{Color32, Id, RichText};
use fedek_core::suggest;

use crate::app::{bold, same_outcomes, App, BolognaPreview, Dialog, FetchTarget, ACCENT, WARN_RED};
use crate::ui::{assessments::parse_roster, danger_button, primary_button, secondary_button, wrapped};

pub fn show(app: &mut App, ctx: &egui::Context) {
    if let Some(f) = &app.fetch {
        let url = f.url.clone();
        let cancel = egui::Modal::new(Id::unique("dlg-fetch")).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(bold("Bologna sayfası indiriliyor…").size(20.0));
            });
            ui.add_space(4.0);
            ui.label(RichText::new(url).size(13.0).color(Color32::DARK_GRAY));
            ui.add_space(10.0);
            secondary_button(ui, "Vazgeç").clicked()
        });
        if cancel.inner {
            app.cancel_fetch();
        }
    }

    let Some(dialog) = app.dialog.take() else { return };
    let mut keep: Option<Dialog> = None;

    match dialog {
        Dialog::Error(msg) => {
            let m = egui::Modal::new(Id::unique("dlg-error")).show(ctx, |ui| {
                ui.set_width(520.0);
                ui.label(bold("Bir sorun oluştu").size(20.0).color(WARN_RED));
                ui.add_space(6.0);
                ui.label(&msg);
                ui.add_space(10.0);
                primary_button(ui, "Tamam").clicked()
            });
            if !(m.inner || m.should_close()) {
                keep = Some(Dialog::Error(msg));
            }
        }
        Dialog::ConfirmDeleteCourse { id, title } => {
            let mut decided = false;
            let m = egui::Modal::new(Id::unique("dlg-del-course")).show(ctx, |ui| {
                ui.set_width(520.0);
                ui.label(bold("Ders silinsin mi?").size(20.0));
                ui.add_space(6.0);
                ui.label(format!("“{}” ve içindeki tüm puanlar kalıcı olarak silinecek.", title.trim()));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if danger_button(ui, "Evet, sil").clicked() {
                        decided = true;
                        app.delete_course(&id);
                    }
                    if secondary_button(ui, "İptal").clicked() {
                        decided = true;
                    }
                });
            });
            if !decided && !m.should_close() {
                keep = Some(Dialog::ConfirmDeleteCourse { id, title });
            }
        }
        Dialog::ConfirmDeleteAssessment(i) => {
            let mut decided = false;
            let name = app.open.as_ref().and_then(|o| o.file.course.assessments.get(i)).map(|a| a.name.clone()).unwrap_or_default();
            let m = egui::Modal::new(Id::unique("dlg-del-assessment")).show(ctx, |ui| {
                ui.set_width(520.0);
                ui.label(bold("Değerlendirme silinsin mi?").size(20.0));
                ui.add_space(6.0);
                ui.label(format!("“{name}” ile birlikte soruları ve öğrenci puanları silinecek."));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if danger_button(ui, "Evet, sil").clicked() {
                        decided = true;
                        if let Some(o) = app.open.as_mut() {
                            if i < o.file.course.assessments.len() {
                                o.file.course.assessments.remove(i);
                            }
                            o.assessment_tab = o.assessment_tab.saturating_sub(1);
                        }
                    }
                    if secondary_button(ui, "İptal").clicked() {
                        decided = true;
                    }
                });
            });
            if !decided && !m.should_close() {
                keep = Some(Dialog::ConfirmDeleteAssessment(i));
            }
        }
        Dialog::ConfirmDeleteProgram(i) => {
            let mut decided = false;
            let name = app.programs.get(i).map(|p| p.name.clone()).unwrap_or_default();
            let m = egui::Modal::new(Id::unique("dlg-del-program")).show(ctx, |ui| {
                ui.set_width(520.0);
                ui.label(bold("Program silinsin mi?").size(20.0));
                ui.add_space(6.0);
                ui.label(format!("“{name}” PÇ listesi silinecek. Mevcut dersler etkilenmez."));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if danger_button(ui, "Evet, sil").clicked() {
                        decided = true;
                        if app.programs.len() > 1 && i < app.programs.len() {
                            app.programs.remove(i);
                        }
                        app.selected_program = 0;
                    }
                    if secondary_button(ui, "İptal").clicked() {
                        decided = true;
                    }
                });
            });
            if !decided && !m.should_close() {
                keep = Some(Dialog::ConfirmDeleteProgram(i));
            }
        }
        Dialog::PasteRoster { assessment, mut text } => {
            let mut decided = false;
            let n_q = app.open.as_ref().and_then(|o| o.file.course.assessments.get(assessment)).map_or(0, |a| a.questions.len());
            let preview = parse_roster(&text, n_q);
            let m = egui::Modal::new(Id::unique("dlg-paste")).show(ctx, |ui| {
                ui.set_width(640.0);
                ui.label(bold("Öğrenci listesini yapıştırın").size(20.0));
                ui.add_space(4.0);
                ui.label(RichText::new("Her satırda bir öğrenci: numara, ad, soyad (OBYS/Excel'den kopyalayıp yapıştırabilirsiniz). Son kelime soyad sayılır.").size(14.0));
                ui.add_space(6.0);
                ui.add(egui::TextEdit::multiline(&mut text).desired_rows(10).desired_width(f32::INFINITY).hint_text("07230000913\tSUDE\tKİY\n07230000879\tILGAZ\tYILDIRIM"));
                ui.add_space(4.0);
                ui.label(RichText::new(format!("{} öğrenci tanındı.", preview.len())).size(14.0));
                if let Some(first) = preview.first() {
                    ui.label(RichText::new(format!("İlk satır → no: {} · ad: {} · soyad: {}", first.number, first.first_name, first.last_name)).size(13.0));
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(!preview.is_empty(), egui::Button::new(RichText::new("Listeye ekle").size(16.0))).clicked() {
                        decided = true;
                        if let Some(o) = app.open.as_mut() {
                            if let Some(a) = o.file.course.assessments.get_mut(assessment) {
                                a.students.extend(preview.iter().cloned());
                                a.students.truncate(200);
                            }
                        }
                    }
                    if secondary_button(ui, "İptal").clicked() {
                        decided = true;
                    }
                });
            });
            if !decided && !m.should_close() {
                keep = Some(Dialog::PasteRoster { assessment, text });
            }
        }
        Dialog::ChooseProgram => {
            let mut decided = false;
            let programs = app.programs.clone();
            let m = egui::Modal::new(Id::unique("dlg-program")).show(ctx, |ui| {
                ui.set_width(600.0);
                ui.label(bold("Ders hangi programa ait?").size(20.0));
                ui.add_space(8.0);
                for p in &programs {
                    if ui.add(egui::Button::new(RichText::new(&p.name).size(16.0)).min_size(egui::vec2(560.0, 40.0)).wrap()).clicked() {
                        decided = true;
                        app.new_course(p.clone());
                    }
                }
                ui.add_space(8.0);
                if secondary_button(ui, "İptal").clicked() {
                    decided = true;
                }
            });
            if !decided && !m.should_close() {
                keep = Some(Dialog::ChooseProgram);
            }
        }
        Dialog::BolognaUrl { mut text, target } => {
            let mut decided = false;
            let m = egui::Modal::new(Id::unique("dlg-bologna-url")).show(ctx, |ui| {
                ui.set_width(720.0);
                ui.label(bold("Bologna bilgi paketi bağlantısı").size(20.0));
                ui.add_space(4.0);
                ui.label(RichText::new("Bilgi paketinde dersin kendi sayfasını açın, tarayıcının adres çubuğundaki bağlantıyı kopyalayıp buraya yapıştırın. Ders adı, sorumlusu, öğrenme çıktıları (ÖÇ), program çıktıları (PÇ) ve ilişki matrisi alınır.").size(14.0));
                ui.add_space(6.0);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .desired_width(f32::INFINITY)
                        .hint_text("https://ebp.ege.edu.tr/DereceProgramlari/Ders/1/2709/…")
                        .background_color(crate::app::INPUT_PINK),
                );
                resp.request_focus();
                let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let ready = !text.trim().is_empty();
                    let download = ui.add_enabled(ready, egui::Button::new(RichText::new("İndir").color(Color32::WHITE).size(17.0)).fill(ACCENT).min_size(egui::vec2(0.0, 40.0))).clicked();
                    if download || (enter && ready) {
                        decided = true;
                        app.start_fetch(&text, target);
                    }
                    if secondary_button(ui, "İptal").clicked() {
                        decided = true;
                    }
                });
            });
            if !decided && !m.should_close() {
                keep = Some(Dialog::BolognaUrl { text, target });
            }
        }
        Dialog::BolognaCourse(boxed) => {
            let mut p: BolognaPreview = *boxed;
            let mut decided = false;
            let mut apply = false;
            let (current_pcs, existing_oc) = match (&app.open, p.target) {
                (Some(o), FetchTarget::OpenCourse) => (Some(o.file.course.program.outcomes.clone()), o.file.course.outcomes.len()),
                _ => (None, 0),
            };
            let pc_differs = current_pcs.as_ref().is_some_and(|cur| !same_outcomes(cur, &p.data.program_outcomes));
            let matching_program = app.programs.iter().find(|pr| same_outcomes(&pr.outcomes, &p.data.program_outcomes)).map(|pr| pr.name.clone());
            let m = egui::Modal::new(Id::unique("dlg-bologna-course")).show(ctx, |ui| {
                ui.set_width(ui.ctx().content_rect().width().min(1180.0) - 40.0);
                ui.label(bold("Bologna sayfasından alınacaklar").size(20.0));
                ui.add_space(4.0);
                ui.label(RichText::new(format!("{} {}", p.data.code, p.data.name)).size(17.0).strong());
                if !p.data.instructor.is_empty() {
                    ui.label(RichText::new(format!("Dersin sorumlusu: {}", p.data.instructor)).size(14.0));
                }
                ui.add_space(6.0);
                if p.target == FetchTarget::OpenCourse {
                    ui.checkbox(&mut p.fill_info, "Ders kodu/adını, sorumlusunu ve ders türünü de sayfadakiyle değiştir");
                    if existing_oc > 0 {
                        ui.label(RichText::new(format!("Dikkat: dersteki mevcut {existing_oc} öğrenme çıktısı silinip sayfadaki {} ÖÇ yazılacak. Soruların ÖÇ etiketleri sıra numarasına göre korunur.", p.data.outcomes.len())).size(14.0).color(WARN_RED));
                    }
                    if pc_differs {
                        ui.checkbox(&mut p.replace_program, format!("Dersin PÇ listesini sayfadaki {} PÇ ile değiştir", p.data.program_outcomes.len()));
                        ui.label(RichText::new("Dersin mevcut PÇ listesi sayfadakinden farklı. Değiştirmezseniz eşleştirme PÇ sıra numarasına göre yapılır.").size(13.0).color(Color32::DARK_GRAY));
                    }
                } else {
                    match &matching_program {
                        Some(name) => ui.label(RichText::new(format!("Program: {name}")).size(14.0)),
                        None => ui.label(RichText::new("Sayfadaki PÇ listesi kayıtlı programlardan hiçbiriyle aynı değil; bu listeyle yeni bir program oluşturulacak (adını PÇ ekranından düzeltebilirsiniz).").size(14.0).color(WARN_RED)),
                    };
                }
                ui.add_space(8.0);
                ui.label(bold("ÖÇ-PÇ ilişki matrisi").size(17.0));
                ui.label(RichText::new("Her satırda en yüksek katkı düzeyi seçili geldi (şablon kuralı 9). Başka bir hücreye tıklayarak değiştirebilirsiniz; seçili hücreye tekrar tıklamak eşleştirmeyi kaldırır. Katkı düzeyi: 1 çok düşük … 5 çok yüksek.").size(13.0).color(Color32::DARK_GRAY));
                ui.add_space(4.0);
                let pcs = p.data.program_outcomes.clone();
                egui::ScrollArea::both().max_height(ui.ctx().content_rect().height() * 0.5).auto_shrink([false, true]).show(ui, |ui| {
                    egui::Grid::new("bologna-matrix").striped(true).spacing([6.0, 6.0]).show(ui, |ui| {
                        ui.label(bold("ÖÇ"));
                        ui.label(bold("Tanım"));
                        for pc in &pcs {
                            ui.label(bold(&pc.code)).on_hover_text(&pc.text);
                        }
                        ui.scope(|ui| {
                            ui.set_min_width(200.0);
                            ui.label(bold("Seçim"));
                        });
                        ui.end_row();
                        for i in 0..p.data.outcomes.len() {
                            ui.label(bold(fedek_core::outcome_code(i)));
                            wrapped(ui, 420.0, RichText::new(&p.data.outcomes[i]).size(14.0));
                            let row = p.data.matrix.get(i).cloned().unwrap_or_default();
                            for (j, pc) in pcs.iter().enumerate() {
                                let v = row.get(j).copied().flatten();
                                let selected = p.picks.get(i).copied().flatten() == Some(j);
                                let label = v.map_or("·".to_string(), |x| x.to_string());
                                let color = if selected { Color32::WHITE } else if v.is_some() { Color32::from_rgb(0x1a, 0x1a, 0x1a) } else { Color32::GRAY };
                                let mut b = egui::Button::new(RichText::new(label).size(16.0).color(color)).min_size(egui::vec2(38.0, 32.0));
                                b = if selected { b.fill(ACCENT) } else if v.is_some() { b.fill(Color32::from_rgb(0xe6, 0xef, 0xf7)) } else { b.fill(Color32::TRANSPARENT).frame(false) };
                                let r = ui.add_enabled(v.is_some(), b).on_hover_text(format!("{} – {}", pc.code, pc.text));
                                if r.clicked() {
                                    if let Some(slot) = p.picks.get_mut(i) {
                                        *slot = if selected { None } else { Some(j) };
                                    }
                                }
                            }
                            ui.vertical(|ui| {
                                ui.set_min_width(200.0);
                                match p.picks.get(i).copied().flatten() {
                                    Some(j) => {
                                        let code = pcs.get(j).map(|x| x.code.clone()).unwrap_or_else(|| format!("PÇ{}", j + 1));
                                        let s = row.get(j).copied().flatten().map_or(String::new(), |x| format!(" · düzey {x}"));
                                        ui.label(RichText::new(format!("{code}{s}")).size(14.0).strong());
                                    }
                                    None => {
                                        ui.label(RichText::new("— yok —").size(14.0).color(Color32::GRAY));
                                    }
                                }
                                let sg = suggest(&row);
                                if sg.tied.len() > 1 {
                                    ui.label(RichText::new("eşit en yüksek değerler").size(12.0).color(WARN_RED)).on_hover_text("Birden çok PÇ aynı en yüksek düzeye sahip; öğretim elemanının tercihine göre seçin.");
                                }
                            });
                            ui.end_row();
                        }
                    });
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "Uygula").clicked() {
                        decided = true;
                        apply = true;
                    }
                    if secondary_button(ui, "İptal").clicked() {
                        decided = true;
                    }
                });
            });
            if apply {
                app.apply_bologna_course(p);
            } else if !decided && !m.should_close() {
                keep = Some(Dialog::BolognaCourse(Box::new(p)));
            }
        }
        Dialog::BolognaProgram { data, url, target } => {
            let mut decided = false;
            let mut apply = false;
            let about = match target {
                FetchTarget::Program(i) => {
                    let (name, n) = app.programs.get(i).map(|p| (p.name.clone(), p.outcomes.len())).unwrap_or_default();
                    format!("“{name}” programının {n} PÇ'si aşağıdaki {} PÇ ile değiştirilecek.", data.outcomes.len())
                }
                FetchTarget::OpenCourse => format!("Açık dersin PÇ listesi aşağıdaki {} PÇ ile değiştirilecek. ÖÇ eşleştirmelerini sonra kontrol edin.", data.outcomes.len()),
                FetchTarget::NewCourse => format!("Bu {} PÇ ile (gerekirse yeni bir program olarak kaydedilip) yeni ders açılacak.", data.outcomes.len()),
            };
            let m = egui::Modal::new(Id::unique("dlg-bologna-program")).show(ctx, |ui| {
                ui.set_width(760.0);
                ui.label(bold("Program çıktıları (PÇ) listesi").size(20.0));
                ui.add_space(4.0);
                if !data.name.is_empty() {
                    ui.label(RichText::new(&data.name).size(16.0).strong());
                }
                ui.label(RichText::new(about).size(14.0));
                ui.add_space(6.0);
                egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                    for po in &data.outcomes {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(bold(&po.code));
                            ui.label(RichText::new(&po.text).size(14.0));
                        });
                    }
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "Uygula").clicked() {
                        decided = true;
                        apply = true;
                    }
                    if secondary_button(ui, "İptal").clicked() {
                        decided = true;
                    }
                });
            });
            if apply {
                app.apply_bologna_program(data, url, target);
            } else if !decided && !m.should_close() {
                keep = Some(Dialog::BolognaProgram { data, url, target });
            }
        }
    }
    if app.dialog.is_none() {
        app.dialog = keep;
    }
}
