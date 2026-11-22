use std::fs::File;
use std::io::Write;
use std::path::Path;

use flate2::write::DeflateEncoder;
use flate2::Compression;
use rhema_import::{
    import_docx, validate_docx, Block, ImportError, ImportKind, ImportOptions, KeyType, Span,
};
use tempfile::tempdir;

/// Escritor ZIP mínimo para montar fixtures `.docx` sintéticas em testes.
fn write_zip(path: &Path, parts: &[(&str, &[u8], bool)]) {
    let mut file = File::create(path).expect("criar zip sintético");
    let mut cd = Vec::new();
    let mut offset = 0u32;

    for (name, data, deflate) in parts {
        let name_bytes = name.as_bytes();
        let (method, payload): (u16, Vec<u8>) = if *deflate {
            let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
            enc.write_all(data).expect("deflate");
            (8, enc.finish().expect("finish deflate"))
        } else {
            (0, data.to_vec())
        };

        let crc = 0u32; // Nosso leitor streaming não exige CRC fixo nos testes sintéticos
        let comp_len = payload.len() as u32;
        let uncomp_len = data.len() as u32;
        let name_len = name_bytes.len() as u16;

        // Local File Header
        file.write_all(&0x0403_4b50u32.to_le_bytes()).unwrap();
        file.write_all(&20u16.to_le_bytes()).unwrap();
        file.write_all(&0u16.to_le_bytes()).unwrap();
        file.write_all(&method.to_le_bytes()).unwrap();
        file.write_all(&0u16.to_le_bytes()).unwrap();
        file.write_all(&0u16.to_le_bytes()).unwrap();
        file.write_all(&crc.to_le_bytes()).unwrap();
        file.write_all(&comp_len.to_le_bytes()).unwrap();
        file.write_all(&uncomp_len.to_le_bytes()).unwrap();
        file.write_all(&name_len.to_le_bytes()).unwrap();
        file.write_all(&0u16.to_le_bytes()).unwrap();
        file.write_all(name_bytes).unwrap();
        file.write_all(&payload).unwrap();

        // Central Directory Header
        cd.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&method.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&crc.to_le_bytes());
        cd.extend_from_slice(&comp_len.to_le_bytes());
        cd.extend_from_slice(&uncomp_len.to_le_bytes());
        cd.extend_from_slice(&name_len.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(&offset.to_le_bytes());
        cd.extend_from_slice(name_bytes);

        offset = offset
            .checked_add(30 + name_bytes.len() as u32 + comp_len)
            .expect("offset");
    }

    let cd_offset = offset;
    let cd_size = cd.len() as u32;
    let count = parts.len() as u16;
    file.write_all(&cd).unwrap();

    // EOCD
    file.write_all(&0x0605_4b50u32.to_le_bytes()).unwrap();
    file.write_all(&0u16.to_le_bytes()).unwrap();
    file.write_all(&0u16.to_le_bytes()).unwrap();
    file.write_all(&count.to_le_bytes()).unwrap();
    file.write_all(&count.to_le_bytes()).unwrap();
    file.write_all(&cd_size.to_le_bytes()).unwrap();
    file.write_all(&cd_offset.to_le_bytes()).unwrap();
    file.write_all(&0u16.to_le_bytes()).unwrap();
}

const STYLES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Ttulo1">
    <w:name w:val="heading 1"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Ttulo2">
    <w:name w:val="heading 2"/>
    <w:pPr><w:outlineLvl w:val="1"/></w:pPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Ttulo3">
    <w:name w:val="heading 3"/>
    <w:pPr><w:outlineLvl w:val="2"/></w:pPr>
  </w:style>
</w:styles>"#;

fn wrap_doc(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing">
  <w:body>{body}</w:body>
</w:document>"#
    )
}

#[test]
fn dictionary_markers_fields_typos_duplicates_gaps_and_formatting() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("dict.docx");

    let body = r#"
      <w:p><w:pPr><w:pStyle w:val="Ttulo1"/></w:pPr><w:r><w:t>Seção A</w:t></w:r></w:p>
      <w:p><w:pPr><w:pStyle w:val="Ttulo2"/></w:pPr>
        <w:r><w:t>[[@Headword+el: λόγος]][[@GreekStrongs:G1]][[@louw: 33.98]][[@TWOT:100]][[@GreekGK:G3364]][[@HebrewGK:H10]][[@CustomTag:abc]] Palavra Um</w:t></w:r>
      </w:p>
      <w:p>
        <w:r><w:rPr><w:b/><w:i/><w:u w:val="single"/><w:vertAlign w:val="superscript"/></w:rPr><w:t>{{field-on:gloss}}termo inventado</w:t></w:r>
        <w:r><w:t>{{field-off:gloss}} ver [[GreekStrong:G2]] e [[HebrewStrongs: H1]] e [[ desbalanceado e ]] solto. Ref (Gn 31.28) e (Jo 3:16).</w:t></w:r>
      </w:p>
      <w:p><w:pPr><w:pStyle w:val="Ttulo2"/></w:pPr>
        <w:r><w:t>[[@Headword+el:λόγος]][[@GreekStrongs:G1]][[@HebrewStrongs:H1]] Duplicado</w:t></w:r>
      </w:p>
      <w:p><w:r><w:t>{{field-on:gloss}}campo sem fechamento até o fim do parágrafo</w:t></w:r></w:p>
      <w:p><w:pPr><w:pStyle w:val="Ttulo2"/></w:pPr>
        <w:r><w:t>[[@GreekStrongs:G4]][[@HebrewStrongs:H3]] Com lacuna</w:t></w:r>
      </w:p>
      <w:p><w:pPr><w:pStyle w:val="Ttulo2"/></w:pPr>
        <w:r><w:t>Título sem etiqueta no nível de entrada</w:t></w:r>
      </w:p>
      <w:p><w:r><w:t>Texto de exemplo para entrada sem etiqueta.</w:t></w:r></w:p>
    "#;

    let doc = wrap_doc(body);
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdExt1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.invalid/recurso" TargetMode="External"/>
</Relationships>"#;

    write_zip(
        &path,
        &[
            ("word/styles.xml", STYLES_XML.as_bytes(), true),
            ("word/_rels/document.xml.rels", rels.as_bytes(), true),
            ("word/document.xml", doc.as_bytes(), true),
        ],
    );

    let res = import_docx(&path, &ImportOptions::new(ImportKind::Dict)).expect("importar dict");
    let r = &res.report;

    // 3 entradas com etiqueta + 1 título no nível de entrada sem etiqueta
    assert_eq!(r.entries, 4);
    assert_eq!(r.entry_level, Some(2));
    assert_eq!(r.hierarchy_by_level.get(&1), Some(&1));
    assert_eq!(r.hierarchy_by_level.get(&2), Some(&4));
    assert_eq!(r.untagged_entries_at_level, 1);
    assert_eq!(r.empty_entries, 1); // "Com lacuna" não tem parágrafos de corpo

    // Todos os tipos reconhecidos + desconhecido preservado
    assert_eq!(r.tags_by_type.get("Headword+el"), Some(&2));
    assert_eq!(r.tags_by_type.get("GreekStrongs"), Some(&3));
    assert_eq!(r.tags_by_type.get("HebrewStrongs"), Some(&2));
    assert_eq!(r.tags_by_type.get("louw"), Some(&1));
    assert_eq!(r.tags_by_type.get("TWOT"), Some(&1));
    assert_eq!(r.tags_by_type.get("GreekGK"), Some(&1));
    assert_eq!(r.tags_by_type.get("HebrewGK"), Some(&1));
    assert_eq!(r.tags_by_type.get("CustomTag"), Some(&1));

    // Tipo suspeito GreekStrong -> GreekStrongs
    let sus = r
        .suspicious_types
        .get("GreekStrong")
        .expect("GreekStrong suspeito");
    assert_eq!(sus.count, 1);
    assert_eq!(sus.suggestion, "GreekStrongs");

    // Desbalanceamento, campos sem fechamento, duplicatas e lacunas Strong
    assert_eq!(r.unbalanced_brackets, 2);
    assert_eq!(r.unclosed_fields, 1);
    assert_eq!(r.duplicate_headwords, 1);
    assert_eq!(r.duplicate_strongs, 1); // G1 repetido
    assert_eq!(r.strong_gaps.greek.unique_count, 2); // G1, G4 -> lacunas G2, G3 (2)
    assert_eq!(r.strong_gaps.greek.gaps, 2);
    assert_eq!(r.strong_gaps.hebrew.unique_count, 2); // H1, H3 -> lacuna H2 (1)
    assert_eq!(r.strong_gaps.hebrew.gaps, 1);

    // Referências não marcadas no corpo e hiperlink externo (só domínio, não seguido)
    assert_eq!(r.unmarked_bible_refs, 2);
    assert_eq!(
        r.external_hyperlinks_by_domain.get("example.invalid"),
        Some(&1)
    );

    // Verifica IR: hierarquia, chaves, formatação inline e campo
    let e0 = &res.entries[0];
    assert_eq!(e0.title_path, vec!["Seção A", "Palavra Um"]);
    assert!(e0.keys.iter().any(|k| k.kind == KeyType::Headword
        && k.lang.as_deref() == Some("el")
        && k.value == "λόγος"));
    assert!(e0
        .keys
        .iter()
        .any(|k| k.kind == KeyType::Unknown("CustomTag".into())));
    let Block::Paragraph { spans, .. } = &e0.blocks[0] else {
        panic!("esperado parágrafo");
    };
    let Span::Text { style, field, .. } = &spans[0] else {
        panic!("esperado span texto");
    };
    assert!(style.bold && style.italic && style.underline && style.superscript);
    assert_eq!(field.as_deref(), Some("gloss"));
}

#[test]
fn commentary_lists_tables_images_and_bible_refs() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("comm.docx");

    let body = r#"
      <w:p><w:pPr><w:pStyle w:val="Ttulo1"/></w:pPr><w:r><w:t>Evangelhos</w:t></w:r></w:p>
      <w:p><w:pPr><w:pStyle w:val="Ttulo2"/></w:pPr><w:r><w:t>[[@Bible: Mt 1.1-17]] Genealogia</w:t></w:r></w:p>
      <w:p><w:r><w:t>[[@Bíblia: At 14.8-20]] Nota com ponto e [[@Bible: 1Co 2.3]] e [[@Bible: LivroInvalido 99.99]].</w:t></w:r></w:p>
      <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Item de lista inventado</w:t></w:r></w:p>
      <w:p><w:r><w:drawing><wp:docPr id="1" name="Mapa" descr="Mapa ilustrativo"/><a:blip r:embed="rIdImg1"/></w:drawing></w:r></w:p>
      <w:tbl>
        <w:tr><w:tc><w:p><w:r><w:t>C1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>C2</w:t></w:r></w:p></w:tc></w:tr>
        <w:tr><w:tc><w:p><w:r><w:t>C3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>C4</w:t></w:r></w:p></w:tc></w:tr>
      </w:tbl>
    "#;
    let doc = wrap_doc(body);
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img1.png"/>
</Relationships>"#;
    let fake_png = vec![0x89, b'P', b'N', b'G', 0, 1, 2, 3, 4, 5];

    write_zip(
        &path,
        &[
            ("word/styles.xml", STYLES_XML.as_bytes(), true),
            ("word/_rels/document.xml.rels", rels.as_bytes(), true),
            ("word/media/img1.png", &fake_png, false),
            ("word/document.xml", doc.as_bytes(), true),
        ],
    );

    let res = import_docx(&path, &ImportOptions::new(ImportKind::Comm)).expect("importar comm");
    let r = &res.report;
    assert_eq!(r.entries, 1);
    assert_eq!(r.bible_refs_resolved, 3); // Mt 1.1-17, At 14.8-20, 1Co 2.3
    assert_eq!(r.bible_refs_unresolved, 1);
    assert_eq!(r.unresolved_ref_examples, vec!["LivroInvalido 99.99"]);
    assert_eq!(r.list_items, 1);
    assert_eq!(r.tables, 1);
    assert_eq!(r.images, 1);

    let blocks = &res.entries[0].blocks;
    // Parágrafo com âncora no início
    assert!(matches!(&blocks[0], Block::Paragraph { anchors, .. } if anchors.len() == 1));
    assert!(matches!(&blocks[1], Block::ListItem { level: 0, .. }));
    assert!(matches!(
        &blocks[2],
        Block::Image {
            target,
            size_bytes: 10,
            ..
        } if target == "word/media/img1.png"
    ));
    assert!(matches!(&blocks[3], Block::Table { rows } if rows.len() == 2 && rows[0].len() == 2));
}

#[test]
fn corrupted_archive_returns_typed_error_without_panic() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("corrompido.docx");
    std::fs::write(&path, b"nao e um zip valido nem docx").unwrap();
    let err = validate_docx(&path, ImportKind::Dict).expect_err("deve falhar");
    assert!(matches!(err, ImportError::InvalidArchive(_)), "{err:?}");
}

#[test]
fn entry_size_limit_rejects_giant_entry() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("gigante.docx");

    let big_text = "x".repeat(4000);
    let body = format!(
        r#"<w:p><w:pPr><w:pStyle w:val="Ttulo1"/></w:pPr><w:r><w:t>[[@Headword:Grande]] Grande</w:t></w:r></w:p>
           <w:p><w:r><w:t>{big_text}</w:t></w:r></w:p>"#
    );
    let doc = wrap_doc(&body);
    write_zip(
        &path,
        &[
            ("word/styles.xml", STYLES_XML.as_bytes(), true),
            ("word/document.xml", doc.as_bytes(), true),
        ],
    );

    let mut opts = ImportOptions::new(ImportKind::Dict);
    opts.max_entry_bytes = 1024;
    let err = import_docx(&path, &opts).expect_err("deve rejeitar entrada gigante");
    assert!(matches!(err, ImportError::EntryTooLarge { .. }), "{err:?}");
}

#[test]
fn table_cell_limit_rejects_oversized_table() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("tabela_grande.docx");

    let cells = "<w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc>".repeat(25);
    let body = format!(
        r#"<w:p><w:pPr><w:pStyle w:val="Ttulo1"/></w:pPr><w:r><w:t>Cap 1</w:t></w:r></w:p>
           <w:tbl><w:tr>{cells}</w:tr></w:tbl>"#
    );
    let doc = wrap_doc(&body);
    write_zip(
        &path,
        &[
            ("word/styles.xml", STYLES_XML.as_bytes(), true),
            ("word/document.xml", doc.as_bytes(), true),
        ],
    );

    let mut opts = ImportOptions::new(ImportKind::Book);
    opts.max_table_cells = 10;
    let err = import_docx(&path, &opts).expect_err("deve rejeitar tabela gigante");
    assert!(matches!(err, ImportError::TableTooLarge { .. }), "{err:?}");
}

#[test]
fn zip_bomb_part_is_rejected_without_unbounded_allocation() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bomba.docx");

    // ~2 MB de espaços comprimem para poucos KB em Deflate; teto no teste = 256 KB
    let bomb_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>{}</w:t></w:r></w:p></w:body></w:document>"#,
        " ".repeat(2 * 1024 * 1024)
    );
    write_zip(&path, &[("word/document.xml", bomb_xml.as_bytes(), true)]);
    let zip_size = std::fs::metadata(&path).unwrap().len();
    assert!(
        zip_size < 32 * 1024,
        "bomba deveria ter poucos KB ({zip_size})"
    );

    let mut opts = ImportOptions::report_only(ImportKind::Book);
    opts.max_part_bytes = 256 * 1024;
    let err = import_docx(&path, &opts).expect_err("deve rejeitar bomba de zip");
    assert!(matches!(err, ImportError::ZipBomb { .. }), "{err:?}");
}

#[test]
fn problems_list_records_all_problem_types_with_truncated_section_title() {
    let dir = tempdir().unwrap();
    let docx_path = dir.path().join("problemas.docx");
    let out_txt = dir.path().join("problemas.txt");

    let long_title =
        "Título muito longo de seção inventada que ultrapassa quarenta caracteres facilmente";
    let body = format!(
        r#"
      <w:p><w:pPr><w:pStyle w:val="Ttulo1"/></w:pPr><w:r><w:t>Seção Principal</w:t></w:r></w:p>
      <w:p><w:pPr><w:pStyle w:val="Ttulo2"/></w:pPr><w:r><w:t>[[@Headword:Alfa]][[@GreekStrongs:G1]] {long_title}</w:t></w:r></w:p>
      <w:p><w:r><w:t>{{{{field-on:gloss}}}}campo aberto e [[GreekStrong:G2]] e [[ solto e [[@Bible: Invalido 99.99]].</w:t></w:r></w:p>
      <w:p><w:pPr><w:pStyle w:val="Ttulo2"/></w:pPr><w:r><w:t>[[@Headword:Alfa]][[@GreekStrongs:G1]] Repetida</w:t></w:r></w:p>
      <w:p><w:pPr><w:pStyle w:val="Ttulo2"/></w:pPr><w:r><w:t>Sem Etiqueta No Nivel</w:t></w:r></w:p>
    "#
    );
    let doc = wrap_doc(&body);
    write_zip(
        &docx_path,
        &[
            ("word/styles.xml", STYLES_XML.as_bytes(), true),
            ("word/document.xml", doc.as_bytes(), true),
        ],
    );

    let mut opts = ImportOptions::report_only(ImportKind::Dict);
    opts.collect_problems = true;
    let res = import_docx(&docx_path, &opts).expect("importar com problemas");
    rhema_import::write_problems_file(&out_txt, &res.problems).expect("gravar problemas");

    let content = std::fs::read_to_string(&out_txt).expect("ler problemas.txt");
    let lines: Vec<&str> = content.lines().collect();
    assert!(lines.iter().any(|l| l.contains("marcador_desbalanceado")));
    assert!(lines.iter().any(|l| l.contains("campo_sem_fechamento")));
    assert!(lines.iter().any(|l| l.contains("tipo_suspeito")));
    assert!(lines.iter().any(|l| l.contains("referencia_nao_resolvida")));
    assert!(lines.iter().any(|l| l.contains("entrada_duplicada")));
    assert!(lines.iter().any(|l| l.contains("titulo_sem_etiqueta")));
    for line in &lines {
        let cols: Vec<&str> = line.split('\t').collect();
        assert!(cols.len() >= 3, "linha malformada: {line}");
        assert!(
            cols[0].starts_with('P'),
            "falta número do parágrafo: {line}"
        );
        assert!(
            cols[2].chars().count() <= 40,
            "título não truncado em 40 chars: {}",
            cols[2]
        );
    }
}
