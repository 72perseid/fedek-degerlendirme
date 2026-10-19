//! Write a [`Course`] as a workbook in the department's Excel template layout.
//!
//! Cell addresses, labels and formulas follow the template so the result can
//! be opened, edited and recalculated in Excel, re-imported by this crate,
//! and handed to anyone who knows the original form. Formula cells also carry
//! the engine's computed value as their cached result, so viewers that do not
//! recalculate (phones, previews) still show the right numbers.

use std::path::Path;

use rust_xlsxwriter::{
    Color, Format, FormatAlign, FormatBorder, Formula, Workbook, Worksheet, XlsxError,
};

use crate::calc::{evaluate, CourseResult};
use crate::import::layout::*;
use crate::model::*;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("Excel dosyası yazılamadı: {0}")]
    Xlsx(#[from] XlsxError),
}

/// Pink: cells the user fills in. Blue: formula cells. Grey: headings.
const INPUT: u32 = 0xF8CBAD;
const FORMULA: u32 = 0x00B0F0;
const HEADING: u32 = 0xD9D9D9;

struct Styles {
    title: Format,
    label: Format,
    input: Format,
    input_wrap: Format,
    input_small: Format,
    formula: Format,
    formula_pct: Format,
    formula_wrap: Format,
    heading: Format,
    note: Format,
    plain: Format,
}

impl Styles {
    fn new() -> Self {
        let base = Format::new().set_border(FormatBorder::Thin).set_font_size(11);
        let fill = |f: &Format, c: u32| f.clone().set_background_color(Color::RGB(c));
        Styles {
            title: fill(&base, FORMULA).set_bold().set_font_size(12),
            label: base.clone().set_bold(),
            input: fill(&base, INPUT).set_bold(),
            input_wrap: fill(&base, INPUT).set_bold().set_text_wrap().set_align(FormatAlign::Top),
            input_small: fill(&base, INPUT).set_font_size(9).set_text_wrap().set_align(FormatAlign::Top),
            formula: fill(&base, FORMULA).set_bold(),
            formula_pct: fill(&base, FORMULA).set_bold().set_num_format("0.00%"),
            formula_wrap: fill(&base, FORMULA).set_bold().set_font_size(9).set_text_wrap().set_align(FormatAlign::Top),
            heading: fill(&base, HEADING).set_bold(),
            note: Format::new().set_text_wrap().set_align(FormatAlign::Top),
            plain: base,
        }
    }
}

const INSTRUCTIONS: [&str; 9] = [
    "Sayfalarda pembe renkli hücrelere veri girişi yapılmalıdır. Mavi renk ile belirtilen hücreler içerisine formül yazıldığı için üzerinde hiçbir işlem yapmayınız veya silmeyiniz. ",
    "Eğer  kopyala-yapıştır seçenekleri kullanarak verileri dosyaya aktarmak istiyorsanız, yapıştırma seçeneklerinden değerleri yapıştır seçeneği tercih edilmelidir.",
    "Hücrelere yapılacak veri girişleri ile ilgili açıklamaları okumak için hücrelerin üzerinde sağ üst köşede belirtilen kırmızı renkteki hücrelere imleci sürükleyebilirsiniz.",
    "Bir ders için birden fazla veya farklı değerlendirme yöntemi kullanılarak dersin öğrenme çıktıları ölçülebilmektedir. Bu nedenle değerlendirme yöntemleri için sayfa2..sayfa13 isimlendirilen sayfaları kullanabilirsiniz.",
    "sayfa2..sayfa13 sayfalarına değerlendirme yönteminin adı, sorunun ilgili olduğu ÖÇ, soru puanı, ÖÇ sağlayan öğrencilerin aldığı en düşük puan ve öğrencilerin sınav sorularından aldığı notların pembe renkli hücrelere girilmesi gerekmektedir.",
    "Öğrenci sayısının çok olduğu dersler için rastgele 30 öğrencinin seçildiği bir örnekleme yapılarak dersin ÖÇ ve PÇ değerlendirmesi yapılabilir.",
    "sayfa1 bölümüne ders bilgilerini, dersin öğrenme çıktıları tanımını, Öçno, Pçno ve dersin başarı değerlendirmenizi pembe renkli hücrelere girmeniz gerekmektedir.",
    "sayfa1 bölümüne dersin bir Öğrenme çıktıları (ÖÇno) ile sadece bir program çıktısı (PÇno) ile eşleştirilmelidir.  Bir program çıktısını birden fazla öğrenme çıktıları ile ölçülüyorsa bu öğrenme çıktıları için aynı PÇno girilebilir. ",
    "ÖÇno-PÇno ilişkisi 1-5 ARASI; 1: Çok düşük, 2: Düşük,3: Orta, 4:Yüksek , 5: Çok yüksek skalası ile tanımlanmalıdır. Bir öğrenme çıktıları kazanımı 3:orta, 4:Yüksek  veya 5: Çok Yüksek olduğu PÇno ile eşleştirlmesi tercih edilmelidir.",
];

const QUESTION_SLOTS: u32 = LAST_Q_COL - FIRST_Q_COL + 1; // 50
const STUDENT_SLOTS: u32 = LAST_STUDENT_ROW - FIRST_STUDENT_ROW + 1; // 200

/// Excel column letters for a 0-based column index.
pub fn col_letter(mut col: u32) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (col % 26) as u8);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

/// Cached display value for a formula. `rust_xlsxwriter` turns an empty
/// result into the number 0, so a blank ("" from IFERROR) is written as a
/// single space: viewers that do not recalculate then show an empty cell,
/// and Excel/LibreOffice recalculate on load anyway (fullCalcOnLoad).
fn num_result(v: Option<f64>) -> String {
    match v {
        Some(v) => format!("{v}"),
        None => " ".to_string(),
    }
}

/// Same convention for string-valued formula results.
fn str_result(s: &str) -> String {
    if s.is_empty() {
        " ".to_string()
    } else {
        s.to_string()
    }
}

fn formula(ws: &mut Worksheet, row: u32, col: u16, text: String, result: impl Into<String>, fmt: &Format) -> Result<(), XlsxError> {
    ws.write_formula_with_format(row, col, Formula::new(text).set_result(result), fmt)?;
    Ok(())
}

/// Sheet name of the i-th assessment (0-based): the template starts at Sayfa2.
pub fn assessment_sheet_name(index: usize) -> String {
    format!("Sayfa{}", index + 2)
}

/// Label the template prints above an assessment column on Sayfa1.
pub fn assessment_slot_label(index: usize) -> String {
    match index {
        0 => "Ara Sınav".to_string(),
        1 => "Dönem Sonu Sınavı".to_string(),
        i => format!("Değerlendirme {}", i - 1),
    }
}

pub fn export_workbook(course: &Course, path: &Path) -> Result<(), ExportError> {
    let mut wb = build(course)?;
    wb.save(path)?;
    Ok(())
}

pub fn export_to_buffer(course: &Course) -> Result<Vec<u8>, ExportError> {
    let mut wb = build(course)?;
    Ok(wb.save_to_buffer()?)
}

fn build(course: &Course) -> Result<Workbook, ExportError> {
    let result = evaluate(course);
    let st = Styles::new();
    let mut wb = Workbook::new();
    write_summary(wb.add_worksheet(), course, &result, &st)?;
    for (i, a) in course.assessments.iter().enumerate() {
        write_assessment(wb.add_worksheet(), course, a, &result, i, &st)?;
    }
    Ok(wb)
}

fn write_summary(ws: &mut Worksheet, course: &Course, res: &CourseResult, st: &Styles) -> Result<(), XlsxError> {
    ws.set_name("Sayfa1")?;
    let n_assess = course.assessments.len();
    let pc_last_row = FIRST_PC_ROW + course.program.outcomes.len().max(1) as u32; // 0-based, inclusive
    let last_assess_col = 7 + n_assess.max(1) as u32 - 1;
    let last_outcome_row = FIRST_OUTCOME_ROW + MAX_OUTCOMES - 1;

    ws.write_string_with_format(0, 1, "DÖNEMLİK FEDEK DERS BAŞARI PÇ DEĞERLENDİRME ", &st.title)?;
    ws.write_string_with_format(1, 3, "Ders bilgilerini giriniz.", &st.label)?;
    for (i, (label, value)) in [
        ("Dersin kodu ve adı", course.code_and_name.as_str()),
        ("Dersin sorumlusu", course.instructor.as_str()),
        ("Dönemi", course.term.as_str()),
        ("Ders türü (Zorunlu/Seçmeli)", course.kind.template_text()),
    ]
    .into_iter()
    .enumerate()
    {
        let r = 2 + i as u32;
        ws.write_string_with_format(r, 1, label, &st.label)?;
        ws.write_string_with_format(r, 2, ":", &st.label)?;
        ws.write_string_with_format(r, 3, value, &st.input)?;
    }

    // PÇ ratios (rows 1-2).
    ws.write_string_with_format(0, 6, "PÇno", &st.formula)?;
    ws.write_string_with_format(1, 6, "Dersin sağladığı PÇ yüzdesi", &st.formula)?;
    for (i, (po, r)) in course.program.outcomes.iter().zip(&res.program_outcomes).enumerate() {
        let c = 7 + i as u16;
        let l = col_letter(c as u32);
        ws.write_string_with_format(0, c, &po.code, &st.formula)?;
        formula(ws, 1, c,
            format!("=IFERROR(SUMIFS($F${a}:$F${b}, $C${a}:$C${b}, {l}1)/SUMIFS($E${a}:$E${b}, $C${a}:$C${b}, {l}1),\"\")",
                a = FIRST_OUTCOME_ROW + 1, b = last_outcome_row + 1),
            num_result(r.ratio), &st.formula_pct)?;
    }

    // ÖÇ averages (rows 4-5).
    ws.write_string_with_format(3, 6, "Öçno", &st.formula)?;
    ws.write_string_with_format(4, 6, "Dersin sağladığı ÖÇ yüzdesi", &st.formula)?;
    for i in 0..course.outcomes.len().max(1) {
        let c = 7 + i as u16;
        let r = FIRST_OUTCOME_ROW + 1 + i as u32;
        ws.write_string_with_format(3, c, &outcome_code(i), &st.formula)?;
        let value = res.outcomes.get(i).and_then(|o| o.average);
        formula(ws, 4, c, format!("=IF($B{r}<>\"\",$G{r},\"\")"), num_result(value), &st.formula_pct)?;
    }

    // Row 6: header line repeated for printing.
    formula(ws, 5, 6, "=D5".into(), course.term.clone(), &st.formula)?;
    formula(ws, 5, 7, "=D3".into(), course.code_and_name.clone(), &st.formula)?;
    formula(ws, 5, 10, "=D6".into(), course.kind.template_text(), &st.formula)?;
    ws.write_string_with_format(5, 11, "DERSİN SORUMLUSU", &st.formula)?;
    formula(ws, 5, 13, "=D4".into(), course.instructor.clone(), &st.formula)?;

    // Instructions.
    ws.write_string_with_format(6, 1, "BİLGİLENDİRME:", &st.label)?;
    for (i, line) in INSTRUCTIONS.iter().enumerate() {
        ws.write_string(7 + i as u32, 0, &format!("{})", i + 1))?;
        ws.write_string_with_format(7 + i as u32, 1, *line, &st.note)?;
    }

    // Instructor's evaluation boxes.
    ws.write_string_with_format(7, 6, "DEĞERLENDİRMENİZ", &st.heading)?;
    ws.write_string_with_format(7, 7, &course.evaluation, &st.input_wrap)?;
    ws.write_string_with_format(8, 6, "İYİLEŞTİRME ÖNERİNİZ", &st.heading)?;
    ws.write_string_with_format(9, 7, &course.improvement, &st.input_wrap)?;
    ws.write_string_with_format(13, 6, "DİĞER DÜŞÜNCELERİNİZ", &st.heading)?;
    ws.write_string_with_format(13, 7, &course.other_notes, &st.input_wrap)?;

    // Assessment slots (rows 17-20).
    ws.write_string_with_format(16, 7, "Dersin öğrenme çıktılarının hesaplanabilmesi için öğrencilerin sorulardan aldıkları puanları alt satırda belirtilen sayfaya giriniz. Kullanmadığınız diğer değerlendirme yöntemleri varsa ilgili sayfa üzerinde işlem yapmayınız.", &st.note)?;
    for i in 0..n_assess {
        let c = 7 + i as u16;
        let sheet = assessment_sheet_name(i);
        ws.write_string_with_format(17, c, &assessment_slot_label(i), &st.formula)?;
        ws.write_string_with_format(18, c, &sheet, &st.label)?;
        formula(ws, 19, c, format!("=IF({sheet}!$C$5=\"\",\"\",{sheet}!$C$5)"), str_result(&course.assessments[i].name), &st.label)?;
    }

    // ÖÇ table.
    ws.write_string_with_format(19, 1, "Dersin öğrenme çıktılarını sırasıyla aşağıdaki sutununa giriniz.", &st.label)?;
    for (c, h) in ["Öçno", "ÖĞRENME ÇIKTILARI TANIMI", "Pçno", "PROGRAM ÇIKTISI TANIMI", "ÖÇno-Pçno ilişkisi", "% PÇ", "ÖÇ ORTALAMA"].iter().enumerate() {
        let fmt = if c >= 5 || c == 3 { &st.formula } else { &st.input_wrap };
        ws.write_string_with_format(20, c as u16, *h, fmt)?;
    }
    for i in 0..n_assess {
        ws.write_string_with_format(20, 7 + i as u16, "ÖÇ", &st.formula)?;
    }
    for i in 0..MAX_OUTCOMES as usize {
        let r = FIRST_OUTCOME_ROW + i as u32;
        let xr = r + 1;
        let lo = course.outcomes.get(i);
        let out = res.outcomes.get(i);
        ws.write_string_with_format(r, 0, &outcome_code(i), &st.input)?;
        ws.write_string_with_format(r, 1, lo.map_or("", |l| l.text.as_str()), &st.input_small)?;
        ws.write_string_with_format(r, 2, lo.and_then(|l| l.program_outcome.as_deref()).unwrap_or(""), &st.input)?;
        let pc_text = lo
            .and_then(|l| l.program_outcome.as_deref())
            .and_then(|code| course.program.outcomes.iter().find(|p| p.code == code))
            .map(|p| p.text.clone())
            .unwrap_or_default();
        formula(ws, r, 3,
            format!("=IF(C{xr}<>\"\",VLOOKUP(C{xr},C${a}:D${b},2,FALSE),\"\")", a = FIRST_PC_ROW, b = pc_last_row + 1),
            str_result(&pc_text), &st.formula_wrap)?;
        match lo.and_then(|l| l.strength) {
            Some(s) => ws.write_number_with_format(r, 4, f64::from(s), &st.input)?,
            None => ws.write_blank(r, 4, &st.input)?,
        };
        formula(ws, r, 5, format!("=IFERROR(E{xr}*G{xr},\"\")"), num_result(out.and_then(|o| o.weighted)), &st.formula_pct)?;
        formula(ws, r, 6,
            format!("=IFERROR(AVERAGE(H{xr}:{}{xr}),\"\")", col_letter(last_assess_col)),
            num_result(out.and_then(|o| o.average)), &st.formula_pct)?;
        for a in 0..n_assess {
            let value = out.and_then(|o| o.per_assessment[a]);
            formula(ws, r, 7 + a as u16,
                format!("={}!$D{}", assessment_sheet_name(a), 8 + i),
                num_result(value), &st.formula_pct)?;
        }
    }

    // PÇ lookup table.
    formula(ws, 44, 3, "=UPPER(D46)".into(), course.program.name.to_uppercase(), &st.formula)?;
    ws.write_string_with_format(45, 3, &course.program.name, &st.input)?;
    ws.write_string_with_format(46, 2, "PÇNO", &st.formula)?;
    ws.write_string_with_format(46, 3, "PROGRAM ÇIKTISI TANIMI", &st.formula)?;
    for (i, po) in course.program.outcomes.iter().enumerate() {
        let r = FIRST_PC_ROW + i as u32;
        ws.write_string_with_format(r, 2, &po.code, &st.input)?;
        ws.write_string_with_format(r, 3, &po.text, &st.input_small)?;
    }

    for (c, w) in [(0, 7.1), (1, 58.4), (2, 8.0), (3, 99.9), (4, 18.6), (5, 10.4), (6, 20.9)] {
        ws.set_column_width(c, w)?;
    }
    ws.set_column_range_width(7, 7 + n_assess.max(1) as u16, 18.6)?;
    ws.set_paper_size(9);
    ws.set_landscape();
    ws.set_print_fit_to_pages(1, 0);
    Ok(())
}

fn write_assessment(ws: &mut Worksheet, course: &Course, a: &Assessment, res: &CourseResult, index: usize, st: &Styles) -> Result<(), XlsxError> {
    ws.set_name(assessment_sheet_name(index))?;
    let ar = &res.assessments[index];
    let first_q = col_letter(FIRST_Q_COL);
    let last_q = col_letter(LAST_Q_COL);
    let (sr0, sr1) = (FIRST_STUDENT_ROW + 1, LAST_STUDENT_ROW + 1); // Excel rows 15, 214

    for (i, (label, src, value)) in [
        ("Dersin kodu ve adı", "D3", course.code_and_name.as_str()),
        ("Dersin sorumlusu", "D4", course.instructor.as_str()),
        ("Dönemi", "D5", course.term.as_str()),
    ]
    .into_iter()
    .enumerate()
    {
        ws.write_string_with_format(i as u32, 0, label, &st.title)?;
        ws.write_string_with_format(i as u32, 1, ":", &st.title)?;
        formula(ws, i as u32, 2, format!("=Sayfa1!{src}"), value, &st.title)?;
    }
    ws.write_string_with_format(3, 2, "Değerlendirme yönteminin adını giriniz.", &st.label)?;
    ws.write_string_with_format(4, 0, "Degerlendirme Adı", &st.label)?;
    ws.write_string_with_format(4, 1, ":", &st.label)?;
    ws.write_string_with_format(4, 2, &a.name, &st.input)?;
    ws.write_string_with_format(5, 3, "Sınavın Ortalama ", &st.label)?;

    for (r, label) in [
        (0, "SORU NO"),
        (1, "SORUNUN İLGİLİ OLDUĞU ÖĞRENME ÇIKTILARI"),
        (3, "ÖĞRENME ÇIKTILARININ SAĞLANMA YÜZDESİ"),
        (4, "ÖĞRENME ÇIKTILARINI SAĞLAYAN ÖĞRENCİ SAYISI"),
        (5, "ÖĞRENCİ SAYISI"),
        (6, "SORU PUANI"),
        (9, "ÖĞRENME ÇIKTILARINI SAĞLAYAN ÖĞRENCİLERİN ALDIĞI EN DÜŞÜK PUAN"),
    ] {
        ws.write_string_with_format(r, 5, label, &st.heading)?;
    }
    ws.write_string_with_format(6, 0, "Öçno", &st.formula)?;
    ws.write_string_with_format(6, 1, ":", &st.formula)?;
    ws.write_string_with_format(6, 2, "ÖĞRENME ÇIKTILARI TANIMI", &st.formula)?;
    ws.write_string_with_format(6, 3, "ÖÇ Yüzdesi ", &st.formula)?;
    ws.write_string_with_format(11, 10, "HER BİR ÖĞRENCİLERİN SORUDAN ALDIĞI PUANI İLGİLİ SUTUNA GİRİNİZ.", &st.heading)?;

    // ÖÇ rows 8..27.
    for i in 0..MAX_OUTCOMES as usize {
        let r = 7 + i as u32;
        ws.write_string_with_format(r, 0, &outcome_code(i), &st.formula)?;
        ws.write_string_with_format(r, 1, ":", &st.formula)?;
        let text = course.outcomes.get(i).map(|l| l.text.clone()).unwrap_or_default();
        formula(ws, r, 2,
            format!("=IF(Sayfa1!B{x}<>\"\",Sayfa1!B{x},\"\")", x = FIRST_OUTCOME_ROW + 1 + i as u32),
            str_result(&text), &st.formula_wrap)?;
        formula(ws, r, 3,
            format!("=IFERROR(AVERAGEIFS({first_q}$4:{last_q}$4, {first_q}$2:{last_q}$2, A{}),\"\")", r + 1),
            num_result(ar.outcome_ratios.get(i).copied().flatten()), &st.formula_pct)?;
    }

    // Question columns: all 50 slots carry formulas, like the template.
    for q in 0..QUESTION_SLOTS {
        let c = (FIRST_Q_COL + q) as u16;
        let l = col_letter(c as u32);
        let question = a.questions.get(q as usize);
        let qr = ar.questions.get(q as usize);
        ws.write_string_with_format(0, c, &format!("SORU{}", q + 1), &st.heading)?;
        match question.and_then(|x| x.outcome) {
            Some(oi) => ws.write_string_with_format(1, c, &outcome_code(oi), &st.input)?,
            None => ws.write_blank(1, c, &st.input)?,
        };
        let tagged = question.map_or(false, |x| x.outcome.is_some());
        formula(ws, 3, c, format!("=IF({l}$2<>\"\",{l}5/{l}6,\"\")"), num_result(qr.and_then(|x| x.ratio)), &st.formula_pct)?;
        formula(ws, 4, c, format!("=IF({l}$2<>\"\",COUNTIF({l}{sr0}:{l}{sr1},\">=\"&{l}10),\"\")"),
            if tagged { num_result(qr.map(|x| x.achieved as f64)) } else { " ".to_string() }, &st.formula)?;
        formula(ws, 5, c,
            format!("=IF({l}$2<>\"\",COUNTIF($I{sr0}:$I{sr1},\"*\")-COUNTIF($J{sr0}:$J{sr1},\"=F2\")-COUNTIF($J{sr0}:$J{sr1},\"=S-\"),\"\")"),
            if tagged { ar.counted_students.to_string() } else { " ".to_string() }, &st.formula)?;
        match question.and_then(|x| x.points) {
            Some(p) => ws.write_number_with_format(6, c, p, &st.input)?,
            None => ws.write_blank(6, c, &st.input)?,
        };
        let threshold = question.and_then(|x| x.threshold);
        formula(ws, 8, c, format!("=IF({l}$2<>\"\",IF({l}$10<>\"\",\"\",\"!\"),\"\")"),
            if tagged && threshold.is_none() { "!" } else { " " }, &st.formula)?;
        match threshold {
            Some(t) => ws.write_number_with_format(9, c, t, &st.input)?,
            None => ws.write_blank(9, c, &st.input)?,
        };
        formula(ws, 12, c, format!("=IF({l}$2<>\"\",{l}1,\"\")"),
            if tagged { format!("SORU{}", q + 1) } else { " ".to_string() }, &st.formula)?;
        let has_scores = a.students.iter().any(|s| matches!(s.scores.get(q as usize), Some(Some(_))));
        formula(ws, 13, c, format!("=IF({l}$2<>\"\",IF(COUNT({l}${sr0}:{l}${sr1})>0,\"\",\"!\"),\"\")"),
            if tagged && !has_scores { "!" } else { " " }, &st.formula)?;
    }

    // Roster header and rows.
    for (c, h) in [(5, "Sıra"), (6, "ÖğrNo"), (7, "Adı"), (8, "Soyadı"), (9, "F2:Devamsız Öğr.")] {
        ws.write_string_with_format(12, c, h, &st.heading)?;
    }
    ws.write_string_with_format(13, 9, "S-:Seçilmeyen Öğr.", &st.heading)?;
    for i in 0..STUDENT_SLOTS {
        let r = FIRST_STUDENT_ROW + i;
        ws.write_number_with_format(r, 5, f64::from(i + 1), &st.plain)?;
        let s = a.students.get(i as usize);
        ws.write_string_with_format(r, 6, s.map_or("", |s| s.number.as_str()), &st.input)?;
        ws.write_string_with_format(r, 7, s.map_or("", |s| s.first_name.as_str()), &st.input)?;
        ws.write_string_with_format(r, 8, s.map_or("", |s| s.last_name.as_str()), &st.input)?;
        ws.write_string_with_format(r, 9, s.and_then(|s| s.status).map_or("", |st| st.template_code()), &st.input)?;
        if let Some(s) = s {
            for (q, score) in s.scores.iter().enumerate() {
                if let Some(v) = score {
                    ws.write_number_with_format(r, (FIRST_Q_COL as usize + q) as u16, *v, &st.input)?;
                }
            }
        }
    }

    for (c, w) in [(0, 23.6), (1, 2.4), (2, 55.4), (3, 10.9), (5, 5.1), (6, 11.0), (7, 23.4), (8, 25.6), (9, 13.0)] {
        ws.set_column_width(c, w)?;
    }
    ws.set_column_range_width(FIRST_Q_COL as u16, LAST_Q_COL as u16, 9.1)?;
    ws.set_paper_size(9);
    ws.set_landscape();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_letters() {
        assert_eq!(col_letter(0), "A");
        assert_eq!(col_letter(10), "K");
        assert_eq!(col_letter(25), "Z");
        assert_eq!(col_letter(26), "AA");
        assert_eq!(col_letter(59), "BH");
    }

    #[test]
    fn slot_labels_follow_the_template() {
        assert_eq!(assessment_slot_label(0), "Ara Sınav");
        assert_eq!(assessment_slot_label(1), "Dönem Sonu Sınavı");
        assert_eq!(assessment_slot_label(2), "Değerlendirme 1");
        assert_eq!(assessment_sheet_name(0), "Sayfa2");
    }
}
