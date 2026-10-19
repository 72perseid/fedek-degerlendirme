//! Core engine for FEDEK course-outcome (ÖÇ) / programme-outcome (PÇ)
//! assessment. Reproduces the arithmetic of the department's Excel template
//! exactly, and reads/writes workbooks in that template's layout.

pub mod bologna;
pub mod calc;
pub mod checks;
pub mod export;
pub mod import;
pub mod model;

pub use bologna::{fetch_page, parse_page, suggest, BolognaCourse, BolognaError, BolognaProgram, Page, Suggestion};
pub use calc::{evaluate, CourseResult};
pub use checks::{run_checks, Check, Severity};
pub use export::{export_to_buffer, export_workbook, ExportError};
pub use import::{import_workbook, ImportError};
pub use model::*;
