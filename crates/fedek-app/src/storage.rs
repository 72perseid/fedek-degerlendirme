//! Where things live on disk. Everything is plain JSON in the user's
//! application-data folder, one file per course, so a course can be backed
//! up or e-mailed by copying a single file.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use fedek_core::{Course, Program, ProgramOutcome};
use serde::{Deserialize, Serialize};

/// A course on disk: the model plus bookkeeping the UI needs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CourseFile {
    pub id: String,
    pub course: Course,
    /// Pass percentage used to pre-fill thresholds (department default 50).
    #[serde(default = "default_pass_percent")]
    pub pass_percent: f64,
    #[serde(default)]
    pub updated_at: String,
}

fn default_pass_percent() -> f64 {
    50.0
}

impl CourseFile {
    pub fn new(course: Course) -> Self {
        CourseFile {
            id: uuid::Uuid::new_v4().to_string(),
            course,
            pass_percent: 50.0,
            updated_at: now(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_zoom")]
    pub zoom: f32,
    #[serde(default)]
    pub last_export_dir: Option<PathBuf>,
    /// Print every student's scores as an appendix of the PDF report.
    #[serde(default = "default_true")]
    pub report_students: bool,
}

fn default_zoom() -> f32 {
    1.2
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            zoom: default_zoom(),
            last_export_dir: None,
            report_students: true,
        }
    }
}

pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

pub struct Storage {
    root: PathBuf,
}

impl Storage {
    pub fn open() -> Result<Self> {
        let root = match ProjectDirs::from("tr", "Arkeoloji", "FEDEK Degerlendirme") {
            Some(d) => d.data_dir().to_path_buf(),
            None => std::env::current_dir()?.join("fedek-data"),
        };
        fs::create_dir_all(root.join("courses")).context("veri klasörü oluşturulamadı")?;
        Ok(Storage { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn course_path(&self, id: &str) -> PathBuf {
        self.root.join("courses").join(format!("{id}.json"))
    }

    pub fn list_courses(&self) -> Vec<CourseFile> {
        let mut out = Vec::new();
        if let Ok(rd) = fs::read_dir(self.root.join("courses")) {
            for entry in rd.flatten() {
                if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                if let Ok(cf) = load_json::<CourseFile>(&entry.path()) {
                    out.push(cf);
                }
            }
        }
        out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        out
    }

    pub fn save_course(&self, cf: &mut CourseFile) -> Result<()> {
        cf.updated_at = now();
        write_json_atomic(&self.course_path(&cf.id), cf)
    }

    pub fn delete_course(&self, id: &str) -> Result<()> {
        let p = self.course_path(id);
        if p.exists() {
            fs::remove_file(&p).with_context(|| format!("silinemedi: {}", p.display()))?;
        }
        Ok(())
    }

    pub fn load_settings(&self) -> Settings {
        load_json(&self.root.join("settings.json")).unwrap_or_default()
    }

    pub fn save_settings(&self, s: &Settings) -> Result<()> {
        write_json_atomic(&self.root.join("settings.json"), s)
    }

    /// Programme presets (PÇ lists). Seeded with the department's own.
    pub fn load_programs(&self) -> Vec<Program> {
        match load_json::<Vec<Program>>(&self.root.join("programs.json")) {
            Ok(p) if !p.is_empty() => p,
            _ => vec![protohistorya_program()],
        }
    }

    pub fn save_programs(&self, programs: &[Program]) -> Result<()> {
        write_json_atomic(&self.root.join("programs.json"), &programs.to_vec())
    }
}

fn load_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).with_context(|| format!("okunamadı: {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("bozuk dosya: {}", path.display()))
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(value)?;
    fs::write(&tmp, text).with_context(|| format!("yazılamadı: {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("kaydedilemedi: {}", path.display()))?;
    Ok(())
}

/// The 8 programme outcomes of Ege University, Protohistorya ve Ön Asya
/// Arkeolojisi, as published in the Bologna information package.
pub fn protohistorya_program() -> Program {
    let texts = [
        "Arkeoloji ile ilgili güncel elektronik veya basılı ders kitapları ve yayınları kullanarak temel bilgilere, kuramsal çerçeve ve yöntemlere ilişkin bilgilere sahip olma.",
        "Arkeolojinin ileri düzeydeki kuramsal ve uygulama bilgilerini gerekli araç gereçleri kullanarak arazide ve ilgili birimde kullanabilme ve çözüm önerileri geliştirebilme becerisi.",
        "Arkeolojik araştırmalarda gerek bağımsız gerekse ekip üyesi olarak sorumluluk alabilme, çalışma yürütme yetkinliğine sahip olma.",
        "Arkeolojide edindiği ileri düzeydeki bilgi ve becerileri kullanarak yorum getirebilme ve farklı bilimsel disiplinlerin bilgilerini arkeolojik sorunları nicel ve nitel veriler ile destekleyerek eleştirel olarak çözmede kullanabilme.",
        "Alanındaki güncel gelişmeleri izlemenin gerekli olduğunun bilincine sahip olarak ve yaşam boyu öğrenme alışkanlığı kazanabilme.",
        "Arkeolojik mirasın (kültür varlıklarının) korunmasını sağlama, toplumsal sorumluluk bilincinin oluşturulması yönünde çalışma, toplumsal ölçekte proje ve etkinlik gerçekleştirebilme.",
        "Arkeolojik ve bilimsel uygulamalar için evrensel olarak kabul görmüş çağdaş yazılım, bilişim ve iletişim teknolojilerini kullanabilme.",
        "Mesleki etik, sosyal adalet, kültürel değerler, iş güvenliği ve ekolojik farkındalık konularında bilince sahip olma.",
    ];
    Program {
        name: "EGE ÜNİVERSİTESİ EDEBİYAT FAKÜLTESİ PROTOHİSTORYA VE ÖN ASYA ARKEOLOJİSİ".to_string(),
        outcomes: texts
            .iter()
            .enumerate()
            .map(|(i, t)| ProgramOutcome {
                code: format!("PÇ{}", i + 1),
                text: t.to_string(),
            })
            .collect(),
        bologna_url: "https://ebp.ege.edu.tr/DereceProgramlari/Detay/1/114/2709/932001".to_string(),
    }
}

/// A file name Windows, macOS and Linux all accept.
pub fn safe_file_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { ' ' } else { c })
        .collect();
    let trimmed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.is_empty() {
        "ders".to_string()
    } else {
        trimmed
    }
}
