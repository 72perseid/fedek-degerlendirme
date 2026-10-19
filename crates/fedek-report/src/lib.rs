//! Printable A4 report of a course's ÖÇ-PÇ assessment, rendered to PDF
//! in-process with the Typst typesetter. No external programs or fonts are
//! needed: the two Noto Sans faces the app uses are embedded here too.
//!
//! The report shows what the Excel template's Sayfa1 shows, at a size that
//! can be read on paper: course header, PÇ attainment, the ÖÇ table with
//! per-assessment percentages, one question table per assessment, the
//! instructor's three evaluation texts, and an appendix with every
//! student's scores.

use std::path::Path;

use fedek_core::{evaluate, outcome_code, Course, CourseResult, StudentStatus};
use thiserror::Error;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst_layout::PagedDocument;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("Rapor dizilemedi: {0}")]
    Typst(String),
    #[error("PDF yazılamadı: {0}")]
    Io(#[from] std::io::Error),
}

/// Everything the report needs besides the course itself.
#[derive(Clone, Debug, Default)]
pub struct ReportOptions {
    /// The course's pass percentage, printed in the header when known.
    pub pass_percent: Option<f64>,
    /// Print the per-student score tables as an appendix.
    pub include_students: bool,
}

/// Render the report to PDF bytes.
pub fn render_pdf(course: &Course, options: &ReportOptions) -> Result<Vec<u8>, ReportError> {
    let (_, pdf) = compile(course, options)?;
    Ok(pdf)
}

/// Render the report and write it to `path`.
pub fn write_pdf(course: &Course, options: &ReportOptions, path: &Path) -> Result<(), ReportError> {
    let pdf = render_pdf(course, options)?;
    std::fs::write(path, pdf)?;
    Ok(())
}

/// Compile and return (page count, PDF bytes).
pub fn compile(course: &Course, options: &ReportOptions) -> Result<(usize, Vec<u8>), ReportError> {
    let source = typst_source(course, options);
    let world = ReportWorld::new(source);
    let compiled = typst::compile::<PagedDocument>(&world);
    let doc = compiled.output.map_err(|errors| {
        ReportError::Typst(errors.iter().map(|e| e.message.to_string()).collect::<Vec<_>>().join("; "))
    })?;
    let pdf = typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default()).map_err(|errors| {
        ReportError::Typst(errors.iter().map(|e| e.message.to_string()).collect::<Vec<_>>().join("; "))
    })?;
    Ok((doc.pages().len(), pdf))
}

// ----- document -----------------------------------------------------------

/// The Typst markup of the report. Public so it can be inspected in tests.
pub fn typst_source(course: &Course, options: &ReportOptions) -> String {
    let res: CourseResult = evaluate(course);
    let mut d = String::with_capacity(16 * 1024);
    let title = if course.code_and_name.trim().is_empty() { "(adsız ders)".to_string() } else { course.code_and_name.trim().to_string() };
    let footer_left = if course.term.trim().is_empty() { title.clone() } else { format!("{title} · {}", course.term.trim()) };

    d.push_str(&format!(
        r##"#set page(paper: "a4", margin: (x: 1.6cm, top: 1.6cm, bottom: 1.8cm), footer: context [
  #set text(size: 8pt, fill: luma(110))
  #grid(columns: (1fr, auto), {footer}, [Sayfa #counter(page).display("1 / 1", both: true)])
])
#set text(font: "Noto Sans", size: 10pt, lang: "tr")
#set par(justify: false, leading: 0.6em)
#set table(stroke: 0.5pt + luma(160), inset: (x: 5pt, y: 4pt), fill: (x, y) => if y == 0 {{ rgb("#e8eef5") }})
#show table.cell.where(y: 0): set text(weight: "bold", size: 9pt)
#show heading.where(level: 1): set text(size: 15pt)
#show heading.where(level: 2): it => {{ v(0.6em); set text(size: 12pt, fill: rgb("#1f5f8b")); it; v(0.2em) }}
#show heading.where(level: 3): set text(size: 10.5pt)
#let pct(v) = v
#let sub(body) = text(size: 8.5pt, fill: luma(90), body)

"##,
        footer = s(&footer_left)
    ));

    // ----- header -----
    d.push_str("#align(center)[#text(size: 15pt, weight: \"bold\")[Ders Öğrenme Çıktıları (ÖÇ) – Program Çıktıları (PÇ)\\ Değerlendirme Raporu]]\n");
    d.push_str(&format!("#align(center)[#text(size: 10pt, fill: luma(70))[{}]]\n#v(0.6em)\n", s(course.program.name.trim())));
    d.push_str("#table(columns: (auto, 1fr), stroke: 0.5pt + luma(160), fill: (x, y) => if x == 0 { rgb(\"#f3f4f6\") },\n");
    let kind = match course.kind {
        fedek_core::CourseKind::Compulsory => "Zorunlu",
        fedek_core::CourseKind::Elective => "Seçmeli",
    };
    d.push_str(&format!("  [*Dersin kodu ve adı*], {},\n", s(&title)));
    d.push_str(&format!("  [*Dersin sorumlusu*], {},\n", s(course.instructor.trim())));
    d.push_str(&format!("  [*Dönemi*], {},\n", s(course.term.trim())));
    d.push_str(&format!("  [*Ders türü*], {},\n", s(kind)));
    let n_students: Vec<String> = course
        .assessments
        .iter()
        .zip(&res.assessments)
        .map(|(a, r)| format!("{}: {} öğrenci ({} hesaba katıldı)", a.name.trim(), a.students.len(), r.counted_students))
        .collect();
    d.push_str(&format!("  [*Değerlendirmeler*], {},\n", s(&n_students.join(" · "))));
    if let Some(p) = options.pass_percent {
        d.push_str(&format!("  [*Eşik (geçme) yüzdesi*], {},\n", s(&format!("%{} – bir sorudan alınan puan, soru puanının bu yüzdesine ulaşıyorsa ilgili ÖÇ sağlanmış sayılır", num(p)))));
    }
    d.push_str(")\n");

    // ----- PÇ table -----
    d.push_str("== 1. Dersin sağladığı program çıktıları\n");
    d.push_str("#table(columns: (auto, 1fr, auto), align: (center, left, right),\n  [PÇ], [Program çıktısı], [Sağlanma],\n");
    for (po, r) in course.program.outcomes.iter().zip(&res.program_outcomes) {
        let v = match r.ratio {
            Some(x) => format!("[*{}*]", pct_s(x)),
            None => "[#sub[ölçülmedi]]".to_string(),
        };
        d.push_str(&format!("  {}, {}, {},\n", s(&po.code), s(po.text.trim()), v));
    }
    d.push_str(")\n");
    d.push_str("#sub[Sağlanma = Σ(ilişki düzeyi × ÖÇ ortalaması) / Σ(ilişki düzeyi), o PÇ ile eşleştirilen ÖÇ'ler üzerinden. Ölçülmeyen ÖÇ'ler paya 0 katar.]\n");

    // ----- ÖÇ table -----
    let n_a = course.assessments.len();
    d.push_str("== 2. Öğrenme çıktıları\n");
    let mut cols = vec!["auto".to_string(), "1fr".to_string(), "auto".to_string(), "auto".to_string()];
    cols.extend(std::iter::repeat("2.1cm".to_string()).take(n_a));
    cols.push("1.9cm".to_string());
    cols.push("1.6cm".to_string());
    let mut align = vec!["center", "left", "center", "center"];
    align.extend(std::iter::repeat("right").take(n_a + 2));
    d.push_str(&format!("#table(columns: ({}), align: ({}),\n", cols.join(", "), align.join(", ")));
    d.push_str("  [ÖÇ], [Öğrenme çıktısı], [PÇ], [Düzey], ");
    for a in &course.assessments {
        d.push_str(&format!("{}, ", s(a.name.trim())));
    }
    d.push_str("[Ortalama], [Düzey × Ort.],\n");
    for (i, (lo, r)) in course.outcomes.iter().zip(&res.outcomes).enumerate() {
        d.push_str(&format!("  {}, {}, {}, {}, ", s(&outcome_code(i)), s(lo.text.trim()), s(lo.program_outcome.as_deref().unwrap_or("–")), s(&lo.strength.map_or("–".to_string(), |x| x.to_string()))));
        for v in &r.per_assessment {
            d.push_str(&format!("{}, ", pct_cell(*v)));
        }
        let avg = match r.average {
            Some(x) => format!("[*{}*]", pct_s(x)),
            None => "[#sub[ölçülmedi]]".to_string(),
        };
        let w = r.weighted.map_or("–".to_string(), |x| num2(x));
        d.push_str(&format!("{}, {},\n", avg, s(&w)));
    }
    d.push_str(")\n");
    d.push_str("#sub[Her sınav için yüzde, o ÖÇ'yi ölçen sorularda eşiği geçen öğrenci oranlarının ortalamasıdır; \"Ortalama\" sınavların ortalamasıdır. Düzey × Ort. şablondaki F sütunudur.]\n");

    // ----- assessments -----
    d.push_str("== 3. Değerlendirmeler ve sorular\n");
    for (k, (a, r)) in course.assessments.iter().zip(&res.assessments).enumerate() {
        let f2 = a.students.iter().filter(|st| st.status == Some(StudentStatus::Absent)).count();
        let sm = a.students.iter().filter(|st| st.status == Some(StudentStatus::NotSampled)).count();
        d.push_str(&format!("=== 3.{} {}\n", k + 1, s_inline(a.name.trim())));
        d.push_str(&format!(
            "#sub[Listede {} öğrenci; sınava girmeyen (F2): {}; örneklem dışı (S-): {}; hesaba katılan: *{}*.]\n",
            a.students.len(),
            f2,
            sm,
            r.counted_students
        ));
        if a.questions.is_empty() {
            d.push_str("#sub[Soru girilmemiş.]\n");
            continue;
        }
        d.push_str("#table(columns: (auto, auto, auto, auto, auto, auto), align: (center, center, right, right, right, right),\n  [Soru], [Ölçtüğü ÖÇ], [Puan], [Eşik puanı], [Eşiği geçen], [Oran],\n");
        for (qi, (q, qr)) in a.questions.iter().zip(&r.questions).enumerate() {
            d.push_str(&format!(
                "  {}, {}, {}, {}, {}, {},\n",
                s(&format!("S{}", qi + 1)),
                s(&q.outcome.map_or("–".to_string(), outcome_code)),
                s(&q.points.map_or("–".to_string(), num)),
                s(&q.threshold.map_or("–".to_string(), num)),
                s(&qr.achieved.to_string()),
                pct_cell(qr.ratio)
            ));
        }
        d.push_str(")\n");
    }

    // ----- evaluation texts -----
    d.push_str("== 4. Değerlendirme ve öneriler\n");
    for (label, text) in [
        ("Değerlendirmeniz", &course.evaluation),
        ("İyileştirme öneriniz", &course.improvement),
        ("Diğer düşünceleriniz", &course.other_notes),
    ] {
        d.push_str(&format!("#block(width: 100%, stroke: 0.5pt + luma(160), inset: 8pt, radius: 3pt, above: 0.8em)[\n  #text(weight: \"bold\")[{label}]\\\n"));
        if text.trim().is_empty() {
            d.push_str("  #sub[—]\n");
        } else {
            for line in text.lines() {
                if line.trim().is_empty() {
                    d.push_str("  #v(0.4em)\n");
                } else {
                    d.push_str(&format!("  #par({})\n", s(line.trim_end())));
                }
            }
        }
        d.push_str("]\n");
    }
    d.push_str("#block(breakable: false, above: 1.6em, width: 100%)[#grid(columns: (1fr, 1fr), gutter: 2em, [Tarih: #box(width: 4cm, stroke: (bottom: 0.5pt))], [Dersin sorumlusu / İmza: #box(width: 6cm, stroke: (bottom: 0.5pt))])]\n");

    // ----- appendix -----
    if options.include_students && course.assessments.iter().any(|a| !a.students.is_empty()) {
        let max_q = course.assessments.iter().map(|a| a.questions.len()).max().unwrap_or(0);
        d.push_str("#pagebreak()\n");
        if max_q > 8 {
            d.push_str("#set page(flipped: true)\n");
        }
        d.push_str("#set text(size: 8.5pt)\n#set table(inset: (x: 4pt, y: 3pt))\n");
        d.push_str("== Ek. Öğrenci puanları\n");
        d.push_str("#sub[Eşik puanını geçen puanlar normal, geçemeyenler gri yazılmıştır. F2: sınava girmedi, S-: örneklem dışı; bu öğrenciler hesaba katılmaz.]\n");
        for (k, (a, _)) in course.assessments.iter().zip(&res.assessments).enumerate() {
            if a.students.is_empty() {
                continue;
            }
            d.push_str(&format!("=== Ek {}. {}\n", k + 1, s_inline(a.name.trim())));
            let n_q = a.questions.len();
            let mut cols = vec!["auto", "auto", "1fr", "1fr", "auto"];
            cols.extend(std::iter::repeat("auto").take(n_q));
            let mut align = vec!["right", "left", "left", "left", "center"];
            align.extend(std::iter::repeat("right").take(n_q));
            d.push_str(&format!("#table(columns: ({}), align: ({}),\n  table.header([\\#], [Öğrenci no], [Adı], [Soyadı], [Durum]", cols.join(", "), align.join(", ")));
            for qi in 0..n_q {
                let tag = a.questions[qi].outcome.map(outcome_code).unwrap_or_default();
                let head = if tag.is_empty() { format!("S{}", qi + 1) } else { format!("S{}\\ {}", qi + 1, tag) };
                d.push_str(&format!(", [{head}]"));
            }
            d.push_str("),\n");
            for (si, st) in a.students.iter().enumerate() {
                let status = st.status.map_or("", |x| x.template_code());
                d.push_str(&format!("  {}, {}, {}, {}, {}", s(&(si + 1).to_string()), s(st.number.trim()), s(st.first_name.trim()), s(st.last_name.trim()), s(status)));
                for qi in 0..n_q {
                    let v = st.scores.get(qi).copied().flatten();
                    let cell = match v {
                        None => "[]".to_string(),
                        Some(x) => {
                            let below = a.questions[qi].threshold.is_some_and(|t| x < t);
                            if below || st.status.is_some() {
                                format!("[#text(fill: luma(120))[{}]]", num(x))
                            } else {
                                s(&num(x))
                            }
                        }
                    };
                    d.push_str(&format!(", {cell}"));
                }
                d.push_str(",\n");
            }
            d.push_str(")\n");
        }
    }
    d
}

/// A Typst string literal.
fn s(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push(' '),
            '\r' | '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Text embedded in markup mode (headings): escape markup characters.
fn s_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if "*_#[]$\\<>@`~/".contains(ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// "%85,3"
fn pct_s(v: f64) -> String {
    format!("%{:.1}", v * 100.0).replace('.', ",")
}

fn pct_cell(v: Option<f64>) -> String {
    match v {
        Some(x) => s(&pct_s(x)),
        None => "[#sub[–]]".to_string(),
    }
}

/// Numbers the way people write them: 25, 33,33, 12,5.
fn num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.2}").trim_end_matches('0').trim_end_matches('.').replace('.', ",")
    }
}

/// Two decimals, Turkish comma: the template's F column.
fn num2(v: f64) -> String {
    format!("{v:.2}").replace('.', ",")
}

// ----- typst world ----------------------------------------------------------

struct ReportWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main: FileId,
    source: Source,
}

impl ReportWorld {
    fn new(text: String) -> Self {
        let fonts: Vec<Font> = [
            &include_bytes!("../../fedek-app/assets/NotoSans-Regular.ttf")[..],
            &include_bytes!("../../fedek-app/assets/NotoSans-Bold.ttf")[..],
        ]
        .into_iter()
        .filter_map(|data| Font::new(Bytes::new(data), 0))
        .collect();
        let main = FileId::new(RootedPath::new(VirtualRoot::Project, VirtualPath::new("rapor.typ").expect("static path")));
        ReportWorld {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(FontBook::from_fonts(&fonts)),
            fonts,
            main,
            source: Source::new(main, text),
        }
    }
}

impl World for ReportWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }
    fn main(&self) -> FileId {
        self.main
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main {
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound("rapor.typ".into()))
        }
    }
    fn file(&self, _id: FileId) -> FileResult<Bytes> {
        Err(FileError::NotFound("rapor.typ".into()))
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
        let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
        Datetime::from_ymd(y, m, d)
    }
}

/// Days since 1970-01-01 to (year, month, day); Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    ((y + if m <= 2 { 1 } else { 0 }) as i32, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Course {
        let path = format!("{}/../../fixtures/golden_anon.xlsx", env!("CARGO_MANIFEST_DIR"));
        fedek_core::import_workbook(Path::new(&path)).expect("fixture imports")
    }

    #[test]
    fn fixture_renders_to_a_multi_page_pdf() {
        let course = fixture();
        let opts = ReportOptions { pass_percent: Some(50.0), include_students: true };
        let (pages, pdf) = compile(&course, &opts).expect("report compiles");
        assert!(pdf.starts_with(b"%PDF"));
        assert!(pages >= 3, "{pages} pages");
        let (pages_short, _) = compile(&course, &ReportOptions { pass_percent: None, include_students: false }).unwrap();
        assert!(pages_short < pages);
    }

    #[test]
    fn awkward_text_is_escaped() {
        let mut course = fixture();
        course.code_and_name = "X\"Y\\Z #not_code [brackets] $math$ 100% */ end".into();
        course.assessments[0].name = "Ara *Sınav* #1 // not a comment".into();
        course.evaluation = "Satır 1\n\nSatır \"3\" \\ test".into();
        let (pages, _) = compile(&course, &ReportOptions::default()).expect("escaped text compiles");
        assert!(pages >= 1);
        let src = typst_source(&course, &ReportOptions::default());
        assert!(src.contains(r#""X\"Y\\Z #not_code [brackets] $math$ 100% */ end""#));
    }

    #[test]
    fn empty_course_still_renders() {
        let course = Course::default();
        let (pages, pdf) = compile(&course, &ReportOptions::default()).expect("empty course compiles");
        assert_eq!(pages, 1);
        assert!(pdf.len() > 1000);
    }

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_000), (2022, 1, 8));
        assert_eq!(civil_from_days(20_745), (2026, 10, 19));
    }
}
