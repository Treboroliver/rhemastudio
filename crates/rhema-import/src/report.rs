use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use serde::Serialize;

use crate::error::ImportError;
use crate::ir::ImportKind;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProblemKind {
    UnbalancedBracket,
    UnclosedField,
    SuspiciousType,
    UntaggedHeading,
    UnresolvedRef,
    DuplicateEntry,
}

impl ProblemKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::UnbalancedBracket => "marcador_desbalanceado",
            Self::UnclosedField => "campo_sem_fechamento",
            Self::SuspiciousType => "tipo_suspeito",
            Self::UntaggedHeading => "titulo_sem_etiqueta",
            Self::UnresolvedRef => "referencia_nao_resolvida",
            Self::DuplicateEntry => "entrada_duplicada",
        }
    }
}

/// Registro de um problema encontrado durante a leitura do `.docx` (para localização no Word).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProblemRecord {
    pub paragraph: usize,
    pub kind: ProblemKind,
    pub section_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Trunca um título de seção em no máximo `max_chars` caracteres, removendo quebras/tabs.
pub fn truncate_section_title(title: &str, max_chars: usize) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return "(sem titulo)".to_string();
    }
    trimmed.chars().take(max_chars).collect()
}

/// Grava a lista de problemas (uma linha por problema) no arquivo indicado.
pub fn write_problems_file(path: &Path, problems: &[ProblemRecord]) -> Result<(), ImportError> {
    let mut file = std::fs::File::create(path)?;
    for item in problems {
        let title = truncate_section_title(&item.section_title, 40);
        if let Some(detail) = item.detail.as_deref() {
            writeln!(
                file,
                "P{}\t{}\t{}\t{}",
                item.paragraph,
                item.kind.label(),
                title,
                detail
            )?;
        } else {
            writeln!(
                file,
                "P{}\t{}\t{}",
                item.paragraph,
                item.kind.label(),
                title
            )?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SuspiciousTypeReport {
    pub count: usize,
    pub suggestion: String,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct StrongRangeStats {
    pub unique_count: usize,
    pub duplicates: usize,
    pub min: Option<u32>,
    pub max: Option<u32>,
    pub gaps: usize,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct StrongGapsReport {
    pub hebrew: StrongRangeStats,
    pub greek: StrongRangeStats,
}

/// Relatório estruturado de validação de um arquivo `.docx` (nunca contém texto de verbete).
#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    pub kind: ImportKind,
    pub entries: usize,
    pub entry_level: Option<u8>,
    pub hierarchy_by_level: BTreeMap<u8, usize>,
    pub paragraphs_by_style: BTreeMap<String, usize>,
    pub keys_by_type: BTreeMap<String, usize>,
    pub tags_by_type: BTreeMap<String, usize>,
    pub links_by_type: BTreeMap<String, usize>,
    pub unbalanced_brackets: usize,
    pub unclosed_fields: usize,
    pub suspicious_types: BTreeMap<String, SuspiciousTypeReport>,
    pub duplicate_headwords: usize,
    pub duplicate_strongs: usize,
    pub strong_gaps: StrongGapsReport,
    pub untagged_entries_at_level: usize,
    pub empty_entries: usize,
    pub bible_refs_resolved: usize,
    pub bible_refs_unresolved: usize,
    pub unresolved_ref_examples: Vec<String>,
    pub unmarked_bible_refs: usize,
    pub external_hyperlinks_by_domain: BTreeMap<String, usize>,
    pub images: usize,
    pub tables: usize,
    pub list_items: usize,
    pub max_entry_chars: usize,
    pub elapsed_ms: f64,
    pub estimated_memory_bytes: usize,
}

impl ValidationReport {
    pub fn format_summary(&self) -> String {
        let mut out = String::new();
        use std::fmt::Write;
        let _ = writeln!(
            out,
            "tipo={:?} entradas={} nivel_entrada={:?} vazias={} sem_etiqueta={}",
            self.kind,
            self.entries,
            self.entry_level,
            self.empty_entries,
            self.untagged_entries_at_level
        );
        let _ = writeln!(out, "hierarquia={:?}", self.hierarchy_by_level);
        let _ = writeln!(out, "chaves={:?}", self.keys_by_type);
        let _ = writeln!(out, "etiquetas={:?}", self.tags_by_type);
        let _ = writeln!(out, "links={:?}", self.links_by_type);
        let _ = writeln!(
            out,
            "desbalanceados={} campos_abertos={} headwords_dup={} strongs_dup={}",
            self.unbalanced_brackets,
            self.unclosed_fields,
            self.duplicate_headwords,
            self.duplicate_strongs
        );
        if !self.suspicious_types.is_empty() {
            let sus: Vec<String> = self
                .suspicious_types
                .iter()
                .map(|(k, v)| format!("{k}({})->{}", v.count, v.suggestion))
                .collect();
            let _ = writeln!(out, "tipos_suspeitos=[{}]", sus.join(", "));
        }
        if self.strong_gaps.hebrew.unique_count > 0 || self.strong_gaps.greek.unique_count > 0 {
            let _ = writeln!(
                out,
                "strong_H(unicos={}, dup={}, faixa={:?}..={:?}, lacunas={}) strong_G(unicos={}, dup={}, faixa={:?}..={:?}, lacunas={})",
                self.strong_gaps.hebrew.unique_count,
                self.strong_gaps.hebrew.duplicates,
                self.strong_gaps.hebrew.min,
                self.strong_gaps.hebrew.max,
                self.strong_gaps.hebrew.gaps,
                self.strong_gaps.greek.unique_count,
                self.strong_gaps.greek.duplicates,
                self.strong_gaps.greek.min,
                self.strong_gaps.greek.max,
                self.strong_gaps.greek.gaps,
            );
        }
        let _ = writeln!(
            out,
            "refs_biblicas(resolvidas={}, nao_resolvidas={}, corpo_sem_marca={})",
            self.bible_refs_resolved, self.bible_refs_unresolved, self.unmarked_bible_refs
        );
        if !self.unresolved_ref_examples.is_empty() {
            let _ = writeln!(
                out,
                "exemplos_nao_resolvidas={:?}",
                self.unresolved_ref_examples
            );
        }
        if !self.external_hyperlinks_by_domain.is_empty() {
            let _ = writeln!(
                out,
                "hiperlinks_externos={:?}",
                self.external_hyperlinks_by_domain
            );
        }
        let _ = writeln!(
            out,
            "imagens={} tabelas={} listas={} maior_verbete_chars={} tempo_ms={:.1} mem_est_kb={}",
            self.images,
            self.tables,
            self.list_items,
            self.max_entry_chars,
            self.elapsed_ms,
            self.estimated_memory_bytes / 1024
        );
        out
    }
}
