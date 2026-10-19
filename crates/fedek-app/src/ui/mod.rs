//! Screens and shared widgets.

pub mod assessments;
pub mod course;
pub mod course_info;
pub mod dialogs;
pub mod home;
pub mod outcomes;
pub mod programs;
pub mod results;

use std::collections::HashMap;

use egui::{Color32, Id, Response, RichText, TextEdit, Ui};

use crate::app::{bold, ACCENT, INPUT_PINK, WARN_RED};

/// "%98,6" or "–".
pub fn pct(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("%{:.1}", x * 100.0).replace('.', ","),
        None => "–".to_string(),
    }
}

/// Numbers the way people write them: 25, 33,33, 12,5.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').replace('.', ",")
    }
}

/// A numeric text cell that tolerates half-typed input ("12," or "1e")
/// without fighting the user. `edits` keeps the raw text of cells that are
/// being typed into; it is dropped when the cell loses focus.
pub fn num_cell(
    ui: &mut Ui,
    edits: &mut HashMap<Id, String>,
    id: Id,
    value: &mut Option<f64>,
    width: f32,
    too_high_above: Option<f64>,
) -> Response {
    let mut text = match edits.get(&id) {
        Some(t) => t.clone(),
        None => value.map(fmt_num).unwrap_or_default(),
    };
    let over = matches!((value.as_ref(), too_high_above), (Some(v), Some(max)) if *v > max);
    let mut te = TextEdit::singleline(&mut text)
        .id(id)
        .desired_width(width)
        .align(egui::Align2::RIGHT_CENTER)
        .background_color(INPUT_PINK);
    if over {
        te = te.text_color(WARN_RED);
    }
    let resp = ui.add(te);
    if resp.changed() {
        let t = text.trim().replace(',', ".");
        if t.is_empty() {
            *value = None;
        } else if let Ok(v) = t.parse::<f64>() {
            *value = Some(v);
        }
        edits.insert(id, text);
    }
    if resp.lost_focus() {
        edits.remove(&id);
    }
    resp
}

pub fn text_field(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, value: &mut String, width: f32) -> Response {
    ui.add(
        TextEdit::singleline(value)
            .id(Id::unique(id))
            .desired_width(width)
            .background_color(INPUT_PINK),
    )
}

pub fn text_area(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, value: &mut String, rows: usize) -> Response {
    ui.add(
        TextEdit::multiline(value)
            .id(Id::unique(id))
            .desired_rows(rows)
            .desired_width(f32::INFINITY)
            .background_color(INPUT_PINK),
    )
}

pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(Color32::WHITE).size(17.0))
            .fill(ACCENT)
            .min_size(egui::vec2(0.0, 40.0)),
    )
}

pub fn secondary_button(ui: &mut Ui, label: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(label).size(16.0)).min_size(egui::vec2(0.0, 36.0)))
}

pub fn danger_button(ui: &mut Ui, label: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(label).color(WARN_RED)).min_size(egui::vec2(0.0, 32.0)))
}

/// One-sentence instruction at the top of a screen.
pub fn hint(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(15.0).color(Color32::from_rgb(0x44, 0x44, 0x44))).wrap());
}

pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(bold(title).size(19.0));
    ui.add_space(2.0);
}

/// Framed white box used for every form section.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(0xd0)))
        .corner_radius(8.0)
        .inner_margin(14.0)
        .show(ui, add)
        .inner
}

/// A label that wraps inside `width`, for table-like grids.
pub fn wrapped(ui: &mut Ui, width: f32, text: impl Into<RichText>) {
    let text: RichText = text.into();
    ui.scope(|ui| {
        ui.set_max_width(width);
        ui.add(egui::Label::new(text).wrap());
    });
}

pub fn strength_label(s: u8) -> &'static str {
    match s {
        1 => "1 · Çok düşük",
        2 => "2 · Düşük",
        3 => "3 · Orta",
        4 => "4 · Yüksek",
        5 => "5 · Çok yüksek",
        _ => "",
    }
}
