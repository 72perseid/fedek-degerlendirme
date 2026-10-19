//! Application state, navigation, autosave and dialogs.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, RichText, TextStyle};
use fedek_core::{BolognaCourse, BolognaProgram, Course, CourseKind, LearningOutcome, Page, Program, ProgramOutcome};

use crate::storage::{safe_file_name, CourseFile, Settings, Storage};
use crate::ui;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Course,
    Programs,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Info,
    Outcomes,
    Assessments,
    Results,
}

impl Step {
    pub const ALL: [Step; 4] = [Step::Info, Step::Outcomes, Step::Assessments, Step::Results];
    pub fn title(self) -> &'static str {
        match self {
            Step::Info => "Ders Bilgileri",
            Step::Outcomes => "Öğrenme Çıktıları (ÖÇ)",
            Step::Assessments => "Sınavlar ve Puanlar",
            Step::Results => "Sonuçlar ve Rapor",
        }
    }
}

/// The course being edited, with what autosave needs.
pub struct Open {
    pub file: CourseFile,
    saved: CourseFile,
    change_seen: Option<Instant>,
    pub step: Step,
    pub assessment_tab: usize,
}

pub enum Dialog {
    Error(String),
    ConfirmDeleteCourse { id: String, title: String },
    ConfirmDeleteAssessment(usize),
    ConfirmDeleteProgram(usize),
    PasteRoster { assessment: usize, text: String },
    ChooseProgram,
    /// Ask for a Bologna package link, then download it.
    BolognaUrl { text: String, target: FetchTarget },
    /// Preview of a downloaded course page before it is applied.
    BolognaCourse(Box<BolognaPreview>),
    /// Preview of a downloaded PÇ list before it is applied.
    BolognaProgram { data: BolognaProgram, url: String, target: FetchTarget },
}

/// Where a downloaded Bologna page should go.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FetchTarget {
    /// Create a new course from the page.
    NewCourse,
    /// Fill the course that is open.
    OpenCourse,
    /// Replace the PÇ list of `programs[i]`.
    Program(usize),
}

/// A download in progress on a background thread.
pub struct Fetch {
    rx: Receiver<Result<Page, String>>,
    pub url: String,
    target: FetchTarget,
}

/// What the course-page preview dialog edits before applying.
pub struct BolognaPreview {
    pub data: BolognaCourse,
    /// Chosen PÇ index per ÖÇ (starts as the strongest cell of the matrix row).
    pub picks: Vec<Option<usize>>,
    pub url: String,
    pub target: FetchTarget,
    /// Also copy course code/name, instructor and course kind.
    pub fill_info: bool,
    /// Replace the course's PÇ list with the page's.
    pub replace_program: bool,
}

/// Command-line start-up options (see `main.rs`).
#[derive(Clone, Debug, Default)]
pub struct StartUp {
    pub open_last: bool,
    pub programs: bool,
    pub step: Option<Step>,
    pub bologna_url: Option<String>,
}

pub struct App {
    pub storage: Storage,
    pub settings: Settings,
    pub programs: Vec<Program>,
    programs_saved: Vec<Program>,
    pub courses: Vec<CourseFile>,
    pub screen: Screen,
    pub open: Option<Open>,
    pub selected_program: usize,
    pub dialog: Option<Dialog>,
    pub fetch: Option<Fetch>,
    status: Option<(String, Instant, bool)>,
    /// In-progress text of numeric cells (see `ui::num_cell`).
    pub num_edits: HashMap<egui::Id, String>,
}

pub const ACCENT: Color32 = Color32::from_rgb(0x1f, 0x5f, 0x8b);
pub const OK_GREEN: Color32 = Color32::from_rgb(0x2e, 0x7d, 0x32);
pub const WARN_RED: Color32 = Color32::from_rgb(0xc6, 0x28, 0x28);
pub const INPUT_PINK: Color32 = Color32::from_rgb(0xfd, 0xf1, 0xea);

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, startup: StartUp) -> Self {
        let storage = Storage::open().expect("veri klasörü açılamadı");
        let settings = storage.load_settings();
        let programs = storage.load_programs();
        setup_style(&cc.egui_ctx, settings.zoom);
        let courses = storage.list_courses();
        let mut app = App {
            programs_saved: programs.clone(),
            storage,
            settings,
            programs,
            courses,
            screen: Screen::Home,
            open: None,
            selected_program: 0,
            dialog: None,
            fetch: None,
            status: None,
            num_edits: HashMap::new(),
        };
        if let Some(url) = &startup.bologna_url {
            app.start_fetch(url, FetchTarget::NewCourse);
        }
        if startup.programs {
            app.screen = Screen::Programs;
        } else if startup.open_last {
            if let Some(cf) = app.courses.first().cloned() {
                app.open_course(cf, startup.step.unwrap_or(Step::Info));
            }
        }
        app
    }

    // ----- Bologna package pages ----------------------------------------

    /// Download a Bologna page on a background thread; `poll_fetch` picks
    /// up the result and opens the matching preview dialog.
    pub fn start_fetch(&mut self, url: &str, target: FetchTarget) {
        let url = url.trim().to_string();
        if url.is_empty() {
            self.fail("Önce Bologna bilgi paketindeki sayfanın bağlantısını yapıştırın.");
            return;
        }
        let (tx, rx) = mpsc::channel();
        let u = url.clone();
        std::thread::spawn(move || {
            let _ = tx.send(fedek_core::fetch_page(&u).map_err(|e| e.to_string()));
        });
        self.fetch = Some(Fetch { rx, url, target });
    }

    pub fn cancel_fetch(&mut self) {
        self.fetch = None;
    }

    fn poll_fetch(&mut self, ctx: &egui::Context) {
        let Some(f) = &self.fetch else { return };
        match f.rx.try_recv() {
            Ok(result) => {
                let f = self.fetch.take().expect("checked above");
                match result {
                    Ok(page) => self.page_received(page, f.url, f.target),
                    Err(e) => self.fail(e),
                }
            }
            Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
            Err(TryRecvError::Disconnected) => {
                self.fetch = None;
                self.fail("İndirme beklenmedik şekilde kesildi.");
            }
        }
    }

    fn page_received(&mut self, page: Page, url: String, target: FetchTarget) {
        match page {
            Page::Course(data) => {
                if let FetchTarget::Program(i) = target {
                    // A course page carries the programme's PÇ list in its matrix header.
                    let name = self.programs.get(i).map(|p| p.name.clone()).unwrap_or_default();
                    let data = BolognaProgram { name, outcomes: data.program_outcomes };
                    self.dialog = Some(Dialog::BolognaProgram { data, url, target });
                    return;
                }
                if data.outcomes.is_empty() {
                    self.fail("Sayfada öğrenme çıktısı (ÖÇ) bulunamadı. Dersin kendi Bologna sayfasının bağlantısını kullandığınızdan emin olun.");
                    return;
                }
                let picks = data.matrix.iter().map(|row| fedek_core::suggest(row).program_outcome).collect();
                let replace_program = match (&self.open, target) {
                    (Some(o), FetchTarget::OpenCourse) => !same_outcomes(&o.file.course.program.outcomes, &data.program_outcomes),
                    _ => false,
                };
                self.dialog = Some(Dialog::BolognaCourse(Box::new(BolognaPreview {
                    data,
                    picks,
                    url,
                    target,
                    fill_info: true,
                    replace_program,
                })));
            }
            Page::Program(data) => self.dialog = Some(Dialog::BolognaProgram { data, url, target }),
        }
    }

    /// The saved programme with this PÇ list, or a new one made from it.
    pub fn program_for(&mut self, pcs: &[ProgramOutcome]) -> Program {
        if let Some(p) = self.programs.iter().find(|p| same_outcomes(&p.outcomes, pcs)) {
            return p.clone();
        }
        let p = Program {
            name: "YENİ PROGRAM (adını PÇ ekranından düzeltin)".into(),
            outcomes: pcs.to_vec(),
            bologna_url: String::new(),
        };
        self.programs.push(p.clone());
        p
    }

    pub fn apply_bologna_course(&mut self, p: BolognaPreview) {
        match p.target {
            FetchTarget::NewCourse => {
                let program = self.program_for(&p.data.program_outcomes);
                let mut course = blank_course(program);
                fill_course(&mut course, &p);
                let mut file = CourseFile::new(course);
                if let Err(e) = self.storage.save_course(&mut file) {
                    self.fail(e.to_string());
                    return;
                }
                self.open_course(file, Step::Outcomes);
            }
            FetchTarget::OpenCourse => {
                if let Some(o) = self.open.as_mut() {
                    fill_course(&mut o.file.course, &p);
                    o.step = Step::Outcomes;
                }
            }
            FetchTarget::Program(_) => {}
        }
        self.notify(format!("Bologna sayfasından {} öğrenme çıktısı alındı. Eşleştirmeleri kontrol edin.", p.data.outcomes.len()));
    }

    pub fn apply_bologna_program(&mut self, data: BolognaProgram, url: String, target: FetchTarget) {
        match target {
            FetchTarget::Program(i) => {
                if let Some(p) = self.programs.get_mut(i) {
                    p.outcomes = data.outcomes.clone();
                    if !data.name.is_empty() {
                        p.name = data.name.clone();
                    }
                    p.bologna_url = url;
                }
            }
            FetchTarget::OpenCourse => {
                if let Some(o) = self.open.as_mut() {
                    o.file.course.program.outcomes = data.outcomes.clone();
                    if !data.name.is_empty() {
                        o.file.course.program.name = data.name.clone();
                    }
                    o.file.course.program.bologna_url = url;
                }
            }
            FetchTarget::NewCourse => {
                let mut program = self.program_for(&data.outcomes);
                if !data.name.is_empty() && program.name.starts_with("YENİ PROGRAM") {
                    program.name = data.name.clone();
                    program.bologna_url = url;
                    if let Some(p) = self.programs.iter_mut().find(|p| same_outcomes(&p.outcomes, &data.outcomes)) {
                        p.name = program.name.clone();
                        p.bologna_url = program.bologna_url.clone();
                    }
                }
                self.new_course(program);
            }
        }
        self.notify(format!("{} program çıktısı (PÇ) alındı.", data.outcomes.len()));
    }

    // ----- status line -------------------------------------------------

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), Instant::now(), false));
    }

    pub fn fail(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        self.status = Some((msg.clone(), Instant::now(), true));
        self.dialog = Some(Dialog::Error(msg));
    }

    // ----- course lifecycle ---------------------------------------------

    pub fn open_course(&mut self, file: CourseFile, step: Step) {
        self.open = Some(Open {
            saved: file.clone(),
            file,
            change_seen: None,
            step,
            assessment_tab: 0,
        });
        self.num_edits.clear();
        self.screen = Screen::Course;
    }

    pub fn new_course(&mut self, program: Program) {
        let mut file = CourseFile::new(blank_course(program));
        if let Err(e) = self.storage.save_course(&mut file) {
            self.fail(e.to_string());
            return;
        }
        self.open_course(file, Step::Info);
    }

    pub fn copy_to_new_term(&mut self, src: &CourseFile) {
        let mut course = src.course.clone();
        course.term.clear();
        course.evaluation.clear();
        course.improvement.clear();
        course.other_notes.clear();
        for a in &mut course.assessments {
            a.students.clear();
        }
        let mut file = CourseFile::new(course);
        file.pass_percent = src.pass_percent;
        if let Err(e) = self.storage.save_course(&mut file) {
            self.fail(e.to_string());
            return;
        }
        self.open_course(file, Step::Info);
    }

    pub fn import_excel(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Excel şablon dosyasını seçin")
            .add_filter("Excel", &["xlsx", "xlsm"])
            .pick_file()
        else {
            return;
        };
        match fedek_core::import_workbook(&path) {
            Ok(course) => {
                let mut file = CourseFile::new(course);
                if let Err(e) = self.storage.save_course(&mut file) {
                    self.fail(e.to_string());
                    return;
                }
                self.notify("Excel dosyası içe aktarıldı.");
                self.open_course(file, Step::Results);
            }
            Err(e) => self.fail(e.to_string()),
        }
    }

    pub fn export_excel(&mut self) {
        let Some(open) = &self.open else { return };
        let c = &open.file.course;
        let name = safe_file_name(&format!("{} {} ÖÇ-PÇ", c.code_and_name, c.term));
        let mut dlg = rfd::FileDialog::new()
            .set_title("Excel dosyasını kaydet")
            .set_file_name(format!("{name}.xlsx"))
            .add_filter("Excel", &["xlsx"]);
        if let Some(dir) = &self.settings.last_export_dir {
            dlg = dlg.set_directory(dir);
        }
        let Some(path) = dlg.save_file() else { return };
        match fedek_core::export_workbook(c, &path) {
            Ok(()) => {
                self.settings.last_export_dir = path.parent().map(|p| p.to_path_buf());
                let _ = self.storage.save_settings(&self.settings);
                self.notify(format!("Excel dosyası kaydedildi: {}", path.display()));
            }
            Err(e) => self.fail(e.to_string()),
        }
    }

    /// Save the printable A4 report and open it in the system's PDF viewer,
    /// where "Yazdır" is one click away.
    pub fn export_pdf(&mut self) {
        let Some(open) = &self.open else { return };
        let c = &open.file.course;
        let name = safe_file_name(&format!("{} {} Rapor", c.code_and_name, c.term));
        let mut dlg = rfd::FileDialog::new()
            .set_title("Raporu PDF olarak kaydet")
            .set_file_name(format!("{name}.pdf"))
            .add_filter("PDF", &["pdf"]);
        if let Some(dir) = &self.settings.last_export_dir {
            dlg = dlg.set_directory(dir);
        }
        let Some(path) = dlg.save_file() else { return };
        let options = fedek_report::ReportOptions {
            pass_percent: Some(open.file.pass_percent),
            include_students: self.settings.report_students,
        };
        match fedek_report::write_pdf(c, &options, &path) {
            Ok(()) => {
                self.settings.last_export_dir = path.parent().map(|p| p.to_path_buf());
                let _ = self.storage.save_settings(&self.settings);
                let opened = open::that_detached(&path).is_ok();
                self.notify(if opened {
                    format!("Rapor kaydedildi ve açıldı: {}", path.display())
                } else {
                    format!("Rapor kaydedildi: {}", path.display())
                });
            }
            Err(e) => self.fail(e.to_string()),
        }
    }

    pub fn delete_course(&mut self, id: &str) {
        if let Err(e) = self.storage.delete_course(id) {
            self.fail(e.to_string());
        }
        self.courses = self.storage.list_courses();
    }

    pub fn go_home(&mut self) {
        self.save_now();
        self.open = None;
        self.courses = self.storage.list_courses();
        self.screen = Screen::Home;
    }

    fn save_now(&mut self) {
        if let Some(open) = &mut self.open {
            if open.file != open.saved {
                match self.storage.save_course(&mut open.file) {
                    Ok(()) => {
                        open.saved = open.file.clone();
                        open.change_seen = None;
                    }
                    Err(e) => self.status = Some((e.to_string(), Instant::now(), true)),
                }
            }
        }
        if self.programs != self.programs_saved {
            match self.storage.save_programs(&self.programs) {
                Ok(()) => self.programs_saved = self.programs.clone(),
                Err(e) => self.status = Some((e.to_string(), Instant::now(), true)),
            }
        }
    }

    /// Save a moment after the last change, so typing is not interrupted.
    fn autosave(&mut self, ctx: &egui::Context) {
        let mut due = false;
        if let Some(open) = &mut self.open {
            if open.file != open.saved {
                match open.change_seen {
                    None => open.change_seen = Some(Instant::now()),
                    Some(t) if t.elapsed() > Duration::from_millis(700) => due = true,
                    _ => {}
                }
                ctx.request_repaint_after(Duration::from_millis(750));
            }
        }
        if self.programs != self.programs_saved {
            due = true;
        }
        if due {
            self.save_now();
        }
    }

    pub fn set_zoom(&mut self, ctx: &egui::Context, zoom: f32) {
        self.settings.zoom = zoom.clamp(0.8, 2.0);
        ctx.set_zoom_factor(self.settings.zoom);
        let _ = self.storage.save_settings(&self.settings);
    }

    pub fn save_state_label(&self) -> (String, Color32) {
        match &self.open {
            Some(o) if o.file != o.saved => ("Kaydediliyor…".into(), Color32::GRAY),
            Some(_) => ("✔ Kaydedildi".into(), OK_GREEN),
            None => (String::new(), Color32::GRAY),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        match self.screen {
            Screen::Home => ui::home::show(self, root),
            Screen::Course => ui::course::show(self, root),
            Screen::Programs => ui::programs::show(self, root),
        }
        self.poll_fetch(&ctx);
        ui::dialogs::show(self, &ctx);
        self.autosave(&ctx);
        if let Some((_, t, is_err)) = &self.status {
            if !is_err && t.elapsed() > Duration::from_secs(6) {
                self.status = None;
            }
        }
    }

    fn on_exit(&mut self) {
        self.save_now();
    }
}

impl App {
    pub fn status_text(&self) -> Option<(String, bool)> {
        self.status.as_ref().map(|(m, _, e)| (m.clone(), *e))
    }
}

/// Big, calm, high-contrast defaults; Noto Sans for proper Turkish glyphs.
fn setup_style(ctx: &egui::Context, zoom: f32) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "noto".into(),
        Arc::new(FontData::from_static(include_bytes!("../assets/NotoSans-Regular.ttf"))),
    );
    fonts.font_data.insert(
        "noto-bold".into(),
        Arc::new(FontData::from_static(include_bytes!("../assets/NotoSans-Bold.ttf"))),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "noto".into());
    fonts
        .families
        .insert(FontFamily::Name("bold".into()), vec!["noto-bold".into(), "noto".into()]);
    ctx.set_fonts(fonts);

    ctx.set_theme(egui::ThemePreference::Light);
    ctx.all_styles_mut(|style| {
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(Color32::from_rgb(0x1a, 0x1a, 0x1a));
    style.visuals.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(0xf3, 0xf4, 0xf6);
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.interact_size.y = 30.0;
    style.text_styles = [
        (TextStyle::Heading, FontId::new(24.0, FontFamily::Name("bold".into()))),
        (TextStyle::Body, FontId::new(16.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(16.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(15.0, FontFamily::Monospace)),
    ]
    .into();
    });
    ctx.set_zoom_factor(zoom);
}

/// A new course the way the department runs them: one midterm, one final.
fn blank_course(program: Program) -> Course {
    Course {
        program,
        kind: CourseKind::Compulsory,
        assessments: vec![
            fedek_core::Assessment { name: "Ara Sınav".into(), ..Default::default() },
            fedek_core::Assessment { name: "Dönem Sonu Sınavı".into(), ..Default::default() },
        ],
        ..Default::default()
    }
}

/// Same PÇ texts in the same order (codes are positional anyway).
pub fn same_outcomes(a: &[ProgramOutcome], b: &[ProgramOutcome]) -> bool {
    let n = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| n(&x.text) == n(&y.text))
}

/// Write a previewed course page into a course: info, PÇ list, ÖÇ list with
/// the chosen PÇ and the matrix strength of that cell.
fn fill_course(course: &mut Course, p: &BolognaPreview) {
    if p.fill_info {
        let code_name = format!("{} {}", p.data.code, p.data.name).trim().to_string();
        if !code_name.is_empty() {
            course.code_and_name = code_name;
        }
        if !p.data.instructor.is_empty() {
            course.instructor = p.data.instructor.clone();
        }
        if let Some(elective) = p.data.elective {
            course.kind = if elective { CourseKind::Elective } else { CourseKind::Compulsory };
        }
    }
    if p.replace_program && !p.data.program_outcomes.is_empty() {
        course.program.outcomes = p.data.program_outcomes.clone();
    }
    course.bologna_url = p.url.clone();
    let outcomes: Vec<LearningOutcome> = p
        .data
        .outcomes
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let pick = p.picks.get(i).copied().flatten();
            let program_outcome = pick.map(|j| {
                course.program.outcomes.get(j).map(|po| po.code.clone()).unwrap_or_else(|| format!("PÇ{}", j + 1))
            });
            let strength = pick.and_then(|j| p.data.matrix.get(i).and_then(|row| row.get(j).copied().flatten()));
            LearningOutcome { text: text.clone(), program_outcome, strength }
        })
        .collect();
    course.outcomes = outcomes;
    let n = course.outcomes.len();
    for a in &mut course.assessments {
        for q in &mut a.questions {
            if q.outcome.is_some_and(|x| x >= n) {
                q.outcome = None;
            }
        }
    }
}

pub fn bold(text: impl Into<String>) -> RichText {
    RichText::new(text).family(FontFamily::Name("bold".into()))
}
