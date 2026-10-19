//! Read course and programme pages of a Bologna information package
//! ("Bilgi Paketi / Ders Kataloğu", the Unipa software used by Ege and
//! many other Turkish universities) so a course can be set up from a link.
//!
//! A course page yields the course code and name, the instructor, the
//! learning outcomes (ÖÇ), the programme outcomes (PÇ, from the matrix
//! header) and the ÖÇ-PÇ contribution matrix. A programme page yields the
//! programme name and its PÇ list.
//!
//! Parsing anchors on the visible section headings, not on element ids, so
//! it tolerates cosmetic changes to the site.

use scraper::{ElementRef, Html, Selector};
use thiserror::Error;

use crate::model::ProgramOutcome;

#[derive(Debug, Error)]
pub enum BolognaError {
    #[error("Bağlantı indirilemedi: {0}")]
    Fetch(String),
    #[error("Bu sayfa bir Bologna ders veya program sayfası olarak tanınmadı. Bilgi paketindeki dersin kendi sayfasının bağlantısını kullanın.")]
    Unrecognized,
    #[error("Sayfada {0} bulunamadı.")]
    Missing(&'static str),
}

/// What a course page contains.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BolognaCourse {
    pub code: String,
    pub name: String,
    pub instructor: String,
    /// "Seçmeli" if the page says the course is elective.
    pub elective: Option<bool>,
    pub outcomes: Vec<String>,
    pub program_outcomes: Vec<ProgramOutcome>,
    /// `matrix[oc][pc]`: contribution level 1..5, `None` when blank.
    pub matrix: Vec<Vec<Option<u8>>>,
}

/// What a programme page contains.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BolognaProgram {
    /// "EGE ÜNİVERSİTESİ EDEBİYAT FAKÜLTESİ PROTOHİSTORYA VE ÖN ASYA ARKEOLOJİSİ"
    pub name: String,
    pub outcomes: Vec<ProgramOutcome>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    Course(BolognaCourse),
    Program(BolognaProgram),
}

/// The PÇ the template should name for one ÖÇ: the strongest cell of the
/// matrix row. `tied` lists every PÇ sharing that strength so the user can
/// be asked to confirm.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Suggestion {
    pub program_outcome: Option<usize>,
    pub strength: Option<u8>,
    pub tied: Vec<usize>,
}

/// Highest strength in the row wins; the first of equal maxima is proposed.
pub fn suggest(row: &[Option<u8>]) -> Suggestion {
    let max = row.iter().flatten().copied().max();
    let Some(max) = max else { return Suggestion::default() };
    let tied: Vec<usize> = row.iter().enumerate().filter(|(_, v)| **v == Some(max)).map(|(i, _)| i).collect();
    Suggestion { program_outcome: tied.first().copied(), strength: Some(max), tied }
}

/// Parse either kind of page.
pub fn parse_page(html: &str) -> Result<Page, BolognaError> {
    let doc = Html::parse_document(html);
    if let Some(c) = parse_course(&doc)? {
        return Ok(Page::Course(c));
    }
    if let Some(p) = parse_program(&doc)? {
        return Ok(Page::Program(p));
    }
    Err(BolognaError::Unrecognized)
}

pub fn parse_course_page(html: &str) -> Result<BolognaCourse, BolognaError> {
    match parse_page(html)? {
        Page::Course(c) => Ok(c),
        Page::Program(_) => Err(BolognaError::Unrecognized),
    }
}

pub fn parse_program_page(html: &str) -> Result<BolognaProgram, BolognaError> {
    match parse_page(html)? {
        Page::Program(p) => Ok(p),
        Page::Course(_) => Err(BolognaError::Unrecognized),
    }
}

// ----- course page ------------------------------------------------------

fn parse_course(doc: &Html) -> Result<Option<BolognaCourse>, BolognaError> {
    // The header table: "Dersin Kodu | Dersin Adı | Dersin Türü | ..."
    let table_sel = sel("table");
    let Some(header) = doc.select(&table_sel).find(|t| {
        let heads: Vec<String> = t.select(&sel("th")).map(|th| norm(&text(th))).collect();
        heads.iter().any(|h| h == "Dersin Kodu") && heads.iter().any(|h| h == "Dersin Adı")
    }) else {
        return Ok(None);
    };
    let head_cols: Vec<String> = header.select(&sel("th")).map(|th| norm(&text(th))).collect();
    let row = header
        .select(&sel("tbody tr"))
        .map(|tr| tr.select(&sel("td")).map(|td| norm(&text(td))).collect::<Vec<_>>())
        .find(|cells| cells.len() >= 2)
        .ok_or(BolognaError::Missing("ders kodu ve adı"))?;
    let col = |name: &str| head_cols.iter().position(|h| h == name);
    let code = row.get(col("Dersin Kodu").unwrap_or(0)).cloned().unwrap_or_default();
    let name = row.get(col("Dersin Adı").unwrap_or(1)).cloned().unwrap_or_default();
    let elective = col("Dersin Türü").and_then(|i| row.get(i)).map(|t| t.to_lowercase().contains("seçmeli"));

    let instructor = heading_section(doc, |t| t.starts_with("Dersi Veren"))
        .and_then(|h| next_element(h, "p"))
        .map(|p| norm(&text(p)))
        .unwrap_or_default();

    let outcomes: Vec<String> = heading_section(doc, |t| t == "Öğrenme Çıktıları")
        .and_then(|h| next_element(h, "table"))
        .map(|t| numbered_rows(t))
        .unwrap_or_default();

    let (program_outcomes, matrix) = match heading_section(doc, |t| t.starts_with("Program ve Öğrenme Çıktıları İlişkisi"))
        .and_then(|h| next_element(h, "table"))
    {
        Some(t) => parse_matrix(t),
        None => (vec![], vec![]),
    };

    // Rows of the matrix follow the ÖÇ list; pad or trim defensively.
    let mut matrix = matrix;
    matrix.resize(outcomes.len(), vec![]);
    for r in &mut matrix {
        r.resize(program_outcomes.len(), None);
    }

    Ok(Some(BolognaCourse { code, name, instructor, elective, outcomes, program_outcomes, matrix }))
}

/// Header cells "PÇ 1".."PÇ n" carry the PÇ text in their `title`; body rows
/// start with "ÖÇ i" followed by one cell per PÇ.
fn parse_matrix(table: ElementRef) -> (Vec<ProgramOutcome>, Vec<Vec<Option<u8>>>) {
    let mut pcs = vec![];
    if let Some(head) = table.select(&sel("thead tr")).next() {
        for cell in head.select(&sel("td, th")) {
            let label = norm(&text(cell));
            if label.is_empty() {
                continue;
            }
            let code = label.replace(' ', "");
            let title = cell.value().attr("title").map(|t| norm(t)).unwrap_or_default();
            pcs.push(ProgramOutcome { code, text: title });
        }
    }
    let mut rows = vec![];
    for tr in table.select(&sel("tbody tr")) {
        let cells: Vec<String> = tr.select(&sel("td")).map(|td| norm(&text(td))).collect();
        if cells.is_empty() {
            continue;
        }
        let values = cells[1..]
            .iter()
            .map(|v| v.parse::<u8>().ok().filter(|n| (1..=5).contains(n)))
            .collect();
        rows.push(values);
    }
    (pcs, rows)
}

// ----- programme page ---------------------------------------------------

fn parse_program(doc: &Html) -> Result<Option<BolognaProgram>, BolognaError> {
    let Some(h) = heading_section(doc, |t| t == "Program Çıktıları") else { return Ok(None) };
    let table = next_element(h, "table").ok_or(BolognaError::Missing("program çıktıları tablosu"))?;
    let outcomes: Vec<ProgramOutcome> = numbered_rows(table)
        .into_iter()
        .enumerate()
        .map(|(i, text)| ProgramOutcome { code: format!("PÇ{}", i + 1), text })
        .collect();
    if outcomes.is_empty() {
        return Err(BolognaError::Missing("program çıktısı"));
    }
    Ok(Some(BolognaProgram { name: program_name(doc), outcomes }))
}

/// "Edebiyat Fakültesi - Protohistorya ve Ön Asya Arkeolojisi (a) -  - Lisans"
/// plus the university from the page title becomes the template's
/// "EGE ÜNİVERSİTESİ EDEBİYAT FAKÜLTESİ PROTOHİSTORYA VE ÖN ASYA ARKEOLOJİSİ".
fn program_name(doc: &Html) -> String {
    let university = doc
        .select(&sel("title"))
        .next()
        .map(|t| norm(&text(t)))
        .and_then(|t| t.split("Bilgi Paketi").next().map(|s| s.trim().to_string()))
        .unwrap_or_default();
    let banner = doc.select(&sel("div.alert h2")).next().map(|h| norm(&text(h))).unwrap_or_default();
    let mut parts: Vec<String> = vec![];
    if !university.is_empty() {
        parts.push(university);
    }
    for piece in banner.split('-') {
        let p = strip_markers(piece);
        if p.is_empty() || matches!(p.as_str(), "Lisans" | "Ön Lisans" | "Yüksek Lisans" | "Doktora") {
            continue;
        }
        parts.push(p);
    }
    turkish_upper(&parts.join(" "))
}

/// Remove "(a)"-style suffixes the catalogue adds to programme variants.
fn strip_markers(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for ch in s.chars() {
        match ch {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    norm(&out)
}

/// Upper-case with Turkish dotted/dotless i.
pub fn turkish_upper(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            'i' => vec!['İ'],
            'ı' => vec!['I'],
            _ => c.to_uppercase().collect(),
        })
        .collect()
}

// ----- shared helpers ---------------------------------------------------

fn sel(css: &str) -> Selector {
    Selector::parse(css).expect("static selector")
}

fn text(el: ElementRef) -> String {
    el.text().collect::<Vec<_>>().join(" ")
}

/// Collapse whitespace (the pages are indented HTML with newlines in cells).
fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// First h2/h3 whose text satisfies `pred`.
fn heading_section<'a>(doc: &'a Html, pred: impl Fn(&str) -> bool) -> Option<ElementRef<'a>> {
    doc.select(&sel("h1, h2, h3, h4")).find(|h| pred(&norm(&text(*h))))
}

/// The next `tag` element after a heading: among the heading's following
/// siblings, or, when the heading is wrapped in its own box, among the
/// box's following siblings. Stops at the next heading.
fn next_element<'a>(heading: ElementRef<'a>, tag: &str) -> Option<ElementRef<'a>> {
    fn scan<'a>(start: ego_tree::NodeRef<'a, scraper::Node>, tag: &str) -> Option<ElementRef<'a>> {
        for node in start.next_siblings() {
            let Some(el) = ElementRef::wrap(node) else { continue };
            let name = el.value().name();
            if name == tag {
                return Some(el);
            }
            if matches!(name, "h1" | "h2" | "h3" | "h4") {
                return None;
            }
            if let Some(found) = el.select(&sel(tag)).next() {
                return Some(found);
            }
        }
        None
    }
    scan(*heading, tag).or_else(|| heading.parent().and_then(|p| scan(p, tag)))
}

/// Rows of "number | text" tables; the text of each row in order.
fn numbered_rows(table: ElementRef) -> Vec<String> {
    table
        .select(&sel("tr"))
        .filter_map(|tr| {
            let cells: Vec<String> = tr.select(&sel("td")).map(|td| norm(&text(td))).collect();
            match cells.len() {
                0 | 1 => None,
                _ => Some(cells[1].clone()),
            }
        })
        .filter(|t| !t.is_empty())
        .collect()
}

// ----- download ---------------------------------------------------------

/// Accept what people paste: surrounding spaces, a missing scheme.
pub fn normalize_url(input: &str) -> String {
    let t = input.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        t.to_string()
    } else {
        format!("https://{t}")
    }
}

/// Download a page (20 s timeout, follows redirects).
pub fn fetch_html(url: &str) -> Result<String, BolognaError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .user_agent("FEDEK-Degerlendirme/0.1 (+egui; Rust)")
        .build()
        .into();
    let mut response = agent.get(&normalize_url(url)).call().map_err(|e| BolognaError::Fetch(describe(e)))?;
    response
        .body_mut()
        .read_to_string()
        .map_err(|e| BolognaError::Fetch(describe(e)))
}

/// Plain-language reasons for the people who will read them.
fn describe(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(404) => "sayfa bulunamadı (404). Bağlantıyı tarayıcıdan yeniden kopyalayın.".into(),
        ureq::Error::StatusCode(code) => format!("sunucu {code} hatası verdi. Biraz sonra yeniden deneyin."),
        ureq::Error::Timeout(_) => "sunucu 20 saniye içinde yanıt vermedi. İnternet bağlantınızı kontrol edin.".into(),
        ureq::Error::ConnectionFailed | ureq::Error::HostNotFound => "sunucuya ulaşılamadı. İnternet bağlantınızı ve adresi kontrol edin.".into(),
        ureq::Error::BadUri(_) => "bu bir internet adresi gibi görünmüyor.".into(),
        other => other.to_string(),
    }
}

/// Download and parse in one step.
pub fn fetch_page(url: &str) -> Result<Page, BolognaError> {
    parse_page(&fetch_html(url)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn course_page_gives_outcomes_pcs_and_matrix() {
        let c = parse_course_page(&fixture("bologna_course_cha001.html")).unwrap();
        assert_eq!(c.code, "CHA001");
        assert_eq!(c.name, "ÇOCUK HAKLARI VE AİLE EĞİTİMİ");
        assert_eq!(c.instructor, "Arş. Gör. Dr. Gürkan ERGÜN");
        assert_eq!(c.elective, Some(false));
        assert_eq!(c.outcomes.len(), 8);
        assert_eq!(c.outcomes[0], "Hukuk ile ilgili temel kavram ve ilkeleri sıralar.");
        assert_eq!(c.outcomes[7], "Aile sorunları ve çocuklara yönelik hizmetlere örnekler verebilir, özel gereksinimli çocuklar ve aileleri ile işbirliğini açıklayabilir.");
        assert_eq!(c.program_outcomes.len(), 8);
        assert_eq!(c.program_outcomes[0].code, "PÇ1");
        assert_eq!(c.program_outcomes[7].text, "Mesleki etik, sosyal adalet, kültürel değerler, iş güvenliği ve ekolojik farkındalık konularında bilince sahip olma.");
        assert_eq!(c.matrix.len(), 8);
        assert_eq!(c.matrix[0], vec![Some(1), Some(1), Some(1), Some(1), Some(5), Some(1), Some(1), Some(5)]);
    }

    #[test]
    fn blank_matrix_cells_and_ties() {
        let c = parse_course_page(&fixture("bologna_course_turkdili2.html")).unwrap();
        assert_eq!(c.code, "715000922009");
        assert_eq!(c.outcomes.len(), 12);
        assert_eq!(c.matrix[0], vec![None, None, Some(4), None, None, None, None, None]);
        assert_eq!(c.matrix[9], vec![None, None, None, Some(4), Some(5), None, None, None]);
        let s = suggest(&c.matrix[9]);
        assert_eq!((s.program_outcome, s.strength), (Some(4), Some(5)));
        assert_eq!(s.tied, vec![4]);
        // ÖÇ7: 3 under PÇ3 and PÇ5 → tie, first proposed.
        let s = suggest(&c.matrix[6]);
        assert_eq!((s.program_outcome, s.strength), (Some(2), Some(3)));
        assert_eq!(s.tied, vec![2, 4]);
        assert_eq!(suggest(&[None, None]), Suggestion::default());
    }

    #[test]
    fn program_page_gives_name_and_pcs() {
        let p = parse_program_page(&fixture("bologna_program_2709.html")).unwrap();
        assert_eq!(p.name, "EGE ÜNİVERSİTESİ EDEBİYAT FAKÜLTESİ PROTOHİSTORYA VE ÖN ASYA ARKEOLOJİSİ");
        assert_eq!(p.outcomes.len(), 8);
        assert_eq!(p.outcomes[0].code, "PÇ1");
        assert!(p.outcomes[0].text.starts_with("Arkeoloji ile ilgili güncel elektronik"));
        // The course page's PÇ texts come from the same source.
        let c = parse_course_page(&fixture("bologna_course_cha001.html")).unwrap();
        assert_eq!(c.program_outcomes, p.outcomes);
    }

    #[test]
    fn pages_are_told_apart() {
        assert!(matches!(parse_page(&fixture("bologna_program_2709.html")), Ok(Page::Program(_))));
        assert!(matches!(parse_page(&fixture("bologna_course_cha001.html")), Ok(Page::Course(_))));
        assert!(matches!(parse_page("<html><body><p>hi</p></body></html>"), Err(BolognaError::Unrecognized)));
    }

    #[test]
    fn helpers() {
        assert_eq!(turkish_upper("ilişki ışık"), "İLİŞKİ IŞIK");
        assert_eq!(normalize_url("  ebp.ege.edu.tr/x "), "https://ebp.ege.edu.tr/x");
        assert_eq!(normalize_url("http://a/b"), "http://a/b");
    }
}
