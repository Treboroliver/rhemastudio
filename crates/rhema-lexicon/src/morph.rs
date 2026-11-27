use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rhema_twformat::{ensure_twm_unprotected, plain_text, DictionarySource};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use walkdir::WalkDir;

use crate::LexiconError;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MorphDecodeResult {
    pub code: String,
    pub found: bool,
    pub explanation: String,
    pub module_id: Option<String>,
    pub module_title: Option<String>,
}

struct MorphSource {
    id: String,
    title: String,
    lang: String,
    key_count: usize,
    source: DictionarySource,
}

/// Decodificador de códigos morfológicos a partir dos dicionários `morph=1` do usuário,
/// com cache em memória e sem tabelas fixas no código.
pub struct MorphDecoder {
    sources: Vec<MorphSource>,
    cache: Mutex<HashMap<(String, String), MorphDecodeResult>>,
}

fn normalize_morph_lang(raw: &str) -> String {
    let cleaned = raw
        .trim()
        .trim_end_matches("<-")
        .trim()
        .to_ascii_lowercase();
    match cleaned.as_str() {
        "pt" | "pt-br" | "pt_br" | "pt-pt" | "por" | "br" | "portuguese" | "português"
        | "portugues" => "por".to_string(),
        "en" | "en-us" | "en-gb" | "eng" | "english" | "inglês" | "ingles" => "eng".to_string(),
        "es" | "spa" | "esp" | "spanish" | "espanhol" => "spa".to_string(),
        other => other.to_string(),
    }
}

fn inspect_morph_config(path: &Path) -> Option<(String, String, usize)> {
    ensure_twm_unprotected(path).ok()?;
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let _ = db.execute_batch("PRAGMA query_only=ON");

    let mut is_morph = false;
    let mut lang = String::new();
    let mut title = String::new();
    {
        let mut stmt = db
            .prepare("SELECT lower(trim(name)), value FROM config WHERE lower(trim(name)) IN ('morph','lang','title','short.title')")
            .ok()?;
        let mut rows = stmt.query([]).ok()?;
        while let Ok(Some(row)) = rows.next() {
            let name: String = row.get(0).unwrap_or_default();
            let val: String = row.get(1).unwrap_or_default();
            let val = val.trim().trim_end_matches("<-").trim().to_string();
            match name.as_str() {
                "morph" if val == "1" => is_morph = true,
                "lang" if !val.is_empty() => lang = normalize_morph_lang(&val),
                "title" if !val.is_empty() => title = val,
                "short.title" if title.is_empty() && !val.is_empty() => title = val,
                _ => {}
            }
        }
    }
    if !is_morph {
        return None;
    }
    let count: usize = db
        .query_row("SELECT COUNT(*) FROM topics", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0)
        .max(0) as usize;
    if title.is_empty() {
        title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Morfologia".into());
    }
    Some((title, lang, count))
}

impl MorphDecoder {
    /// Abre todos os dicionários `morph=1` legíveis e não protegidos a partir de uma lista de caminhos.
    pub fn from_paths(paths: &[PathBuf]) -> Self {
        let mut seen = BTreeSet::new();
        let mut sources = Vec::new();
        for path in paths {
            let Ok(canon) = path.canonicalize() else {
                continue;
            };
            if !seen.insert(canon.clone()) {
                continue;
            }
            let Some((title, lang, key_count)) = inspect_morph_config(&canon) else {
                continue;
            };
            let Ok(source) = DictionarySource::open(&canon) else {
                continue;
            };
            let id = canon
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| title.clone());
            sources.push(MorphSource {
                id,
                title,
                lang,
                key_count,
                source,
            });
        }
        // Português primeiro; depois mais chaves; depois título
        sources.sort_by(|a, b| {
            let rank = |l: &str| match l {
                "por" => 0u8,
                "eng" => 1,
                _ => 2,
            };
            rank(&a.lang)
                .cmp(&rank(&b.lang))
                .then_with(|| b.key_count.cmp(&a.key_count))
                .then_with(|| a.title.cmp(&b.title))
        });
        Self {
            sources,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Varre pastas procurando dicionários `.dct.twm` com `morph=1`.
    pub fn from_folders(roots: &[PathBuf]) -> Self {
        let mut paths = Vec::new();
        for root in roots {
            if !root.is_dir() {
                continue;
            }
            for item in WalkDir::new(root).follow_links(false).into_iter().flatten() {
                if item.file_type().is_file()
                    && item
                        .file_name()
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .ends_with(".dct.twm")
                {
                    paths.push(item.into_path());
                }
            }
        }
        Self::from_paths(&paths)
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// Decodifica um código morfológico usando os dicionários `morph=1` (português primeiro, ou
    /// `lang` preferencial se informado). Código sem entrada devolve `found=false` e o código cru.
    pub fn decode(
        &self,
        raw_code: &str,
        lang: Option<&str>,
    ) -> Result<MorphDecodeResult, LexiconError> {
        let code = raw_code.trim();
        if code.is_empty() || code.chars().count() > 64 || code.chars().any(char::is_control) {
            return Err(LexiconError::Invalid("Código morfológico inválido".into()));
        }
        let pref_lang = lang.map(normalize_morph_lang).unwrap_or_default();
        let cache_key = (code.to_string(), pref_lang.clone());

        if let Ok(guard) = self.cache.lock() {
            if let Some(hit) = guard.get(&cache_key) {
                return Ok(hit.clone());
            }
        }

        // Ordena índices de fontes: idioma pedido > português > demais
        let mut order: Vec<usize> = (0..self.sources.len()).collect();
        if !pref_lang.is_empty() {
            order.sort_by_key(|&i| {
                let l = self.sources[i].lang.as_str();
                if l == pref_lang {
                    0u8
                } else if l == "por" {
                    1
                } else {
                    2
                }
            });
        }

        let mut result = MorphDecodeResult {
            code: code.to_string(),
            found: false,
            explanation: code.to_string(),
            module_id: None,
            module_title: None,
        };

        for idx in order {
            let src = &self.sources[idx];
            if let Ok(Some(entry)) = src.source.lookup(code) {
                if let Ok(nodes) = entry.content {
                    let text = collapse_whitespace(&plain_text(&nodes));
                    if !text.is_empty() {
                        result = MorphDecodeResult {
                            code: code.to_string(),
                            found: true,
                            explanation: text,
                            module_id: Some(src.id.clone()),
                            module_title: Some(src.title.clone()),
                        };
                        break;
                    }
                }
            }
        }

        if let Ok(mut guard) = self.cache.lock() {
            guard.insert(cache_key, result.clone());
        }
        Ok(result)
    }
}

fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tempfile::tempdir;

    fn create_synthetic_morph_dict(path: &Path, title: &str, lang: &str, entries: &[(&str, &str)]) {
        let db = Connection::open(path).unwrap();
        db.execute_batch(
            "CREATE TABLE config(name TEXT, value TEXT);
             CREATE TABLE topics(id INTEGER PRIMARY KEY, pid INTEGER, subject TEXT, rel_order INTEGER);
             CREATE TABLE content(topic_id INTEGER PRIMARY KEY, data BLOB, content_type TEXT);",
        )
        .unwrap();
        db.execute("INSERT INTO config VALUES ('title', ?1)", [title])
            .unwrap();
        db.execute("INSERT INTO config VALUES ('lang', ?1)", [lang])
            .unwrap();
        db.execute("INSERT INTO config VALUES ('morph', '1')", [])
            .unwrap();
        db.execute("INSERT INTO config VALUES ('type', '1')", [])
            .unwrap();
        db.execute("INSERT INTO config VALUES ('content.type', 'rtf')", [])
            .unwrap();
        for (i, (key, desc)) in entries.iter().enumerate() {
            let id = (i + 1) as i64;
            db.execute(
                "INSERT INTO topics(id, pid, subject, rel_order) VALUES (?1, 0, ?2, ?1)",
                rusqlite::params![id, key],
            )
            .unwrap();
            let rtf = format!(r"{{\rtf1\ansi\pard {desc}}}").into_bytes();
            db.execute(
                "INSERT INTO content(topic_id, data, content_type) VALUES (?1, ?2, 'rtf')",
                rusqlite::params![id, rtf],
            )
            .unwrap();
        }
    }

    #[test]
    fn decodes_from_synthetic_morph_dict_prefers_portuguese_caches_and_falls_back_to_raw() {
        let dir = tempdir().unwrap();
        let pt_path = dir.path().join("robinson_pt.dct.twm");
        let en_path = dir.path().join("robinson_en.dct.twm");
        create_synthetic_morph_dict(
            &pt_path,
            "Morfologia PT",
            "por",
            &[("N-NSF", "Substantivo, nominativo, singular, feminino")],
        );
        create_synthetic_morph_dict(
            &en_path,
            "Morphology EN",
            "eng",
            &[
                ("N-NSF", "Noun, nominative, singular, feminine"),
                ("V-AAI-3S", "Verb, aorist, active, indicative, 3rd singular"),
            ],
        );

        let decoder = MorphDecoder::from_folders(&[dir.path().to_path_buf()]);
        assert_eq!(decoder.source_count(), 2);

        // Português primeiro por padrão
        let r1 = decoder.decode("N-NSF", None).unwrap();
        assert!(r1.found);
        assert_eq!(
            r1.explanation,
            "Substantivo, nominativo, singular, feminino"
        );
        assert_eq!(r1.module_title.as_deref(), Some("Morfologia PT"));

        // Segunda consulta vem do cache
        let r1_cached = decoder.decode("N-NSF", None).unwrap();
        assert_eq!(r1_cached, r1);

        // Idioma explícito 'en' prefere inglês
        let r_en = decoder.decode("N-NSF", Some("en")).unwrap();
        assert!(r_en.found);
        assert_eq!(r_en.explanation, "Noun, nominative, singular, feminine");

        // Fallback para outro idioma ativo quando não existe em PT
        let r2 = decoder.decode("V-AAI-3S", None).unwrap();
        assert!(r2.found);
        assert_eq!(
            r2.explanation,
            "Verb, aorist, active, indicative, 3rd singular"
        );

        // Código sem entrada devolve código cru
        let miss = decoder.decode("X-UNKNOWN", None).unwrap();
        assert!(!miss.found);
        assert_eq!(miss.explanation, "X-UNKNOWN");
        assert!(miss.module_id.is_none());

        // Validação
        assert!(decoder.decode("", None).is_err());
    }

    #[test]
    #[ignore = "verificação E3 morfologia com módulos reais via RHEMA_TEST_MODULES"]
    fn real_modules_e3_morph_verification() {
        let Ok(root) = std::env::var("RHEMA_TEST_MODULES") else {
            return;
        };
        let root_path = PathBuf::from(root);
        let decoder = MorphDecoder::from_folders(std::slice::from_ref(&root_path));
        println!("MORPH_SOURCES={}", decoder.source_count());
        assert!(decoder.source_count() >= 1);

        let sample_codes = [
            "N-NSF",
            "N-GSF",
            "N-GSM",
            "N-PRI",
            "V-AAI-3S",
            "V-PAI-3S",
            "A-NSM",
            "P-ASM",
            "T-NSM",
            "CONJ",
            "PREP",
            "ADV",
            "V-2AAI-3S",
            "N-NSM",
            "N-ASM",
            "X-NAO-EXISTE",
        ];
        // Aquece e mede 200 consultas (mistura de hits e misses)
        let mut times_ms = Vec::with_capacity(200);
        for i in 0..200 {
            let code = sample_codes[i % sample_codes.len()];
            let t0 = Instant::now();
            let _ = decoder.decode(code, None).unwrap();
            times_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        times_ms.sort_by(|a, b| a.total_cmp(b));
        println!(
            "MORPH_LATENCY_200: p50={:.3}ms p95={:.3}ms max={:.3}ms",
            times_ms[100],
            times_ms[190],
            times_ms.last().unwrap()
        );
        assert!(times_ms[190] < 50.0);
    }
}
