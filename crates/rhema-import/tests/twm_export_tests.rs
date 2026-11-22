use std::fs::File;
use std::io::Write;
use std::path::Path;

use flate2::write::DeflateEncoder;
use flate2::Compression;
use rhema_core::{Book, LinkTarget, ModuleKind, Node, VerseRef};
use rhema_import::{
    import_docx, write_twm, ImportError, ImportKind, ImportOptions, TwmExportOptions,
};
use rhema_library::{Library, ModuleState};
use rhema_twformat::{
    ensure_twm_unprotected, plain_text, CommentarySource, DictionarySource, TwmModule,
};
use rusqlite::{Connection, OpenFlags};
use tempfile::tempdir;

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

        let comp_len = payload.len() as u32;
        let uncomp_len = data.len() as u32;
        let name_len = name_bytes.len() as u16;

        file.write_all(&0x0403_4b50u32.to_le_bytes()).unwrap();
        file.write_all(&20u16.to_le_bytes()).unwrap();
        file.write_all(&0u16.to_le_bytes()).unwrap();
        file.write_all(&method.to_le_bytes()).unwrap();
        file.write_all(&0u16.to_le_bytes()).unwrap();
        file.write_all(&0u16.to_le_bytes()).unwrap();
        file.write_all(&0u32.to_le_bytes()).unwrap();
        file.write_all(&comp_len.to_le_bytes()).unwrap();
        file.write_all(&uncomp_len.to_le_bytes()).unwrap();
        file.write_all(&name_len.to_le_bytes()).unwrap();
        file.write_all(&0u16.to_le_bytes()).unwrap();
        file.write_all(name_bytes).unwrap();
        file.write_all(&payload).unwrap();

        cd.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&method.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u32.to_le_bytes());
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

        offset += 30 + name_bytes.len() as u32 + comp_len;
    }

    let cd_offset = offset;
    let cd_size = cd.len() as u32;
    let count = parts.len() as u16;
    file.write_all(&cd).unwrap();
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
    <w:name w:val="heading 1"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Ttulo2">
    <w:name w:val="heading 2"/><w:pPr><w:outlineLvl w:val="1"/></w:pPr>
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

fn collect_links(nodes: &[Node], out: &mut Vec<LinkTarget>) {
    for n in nodes {
        match n {
            Node::Link { target, children } => {
                out.push(target.clone());
                collect_links(children, out);
            }
            Node::Bold(c)
            | Node::Italic(c)
            | Node::Underline(c)
            | Node::Superscript(c)
            | Node::Subscript(c)
            | Node::Heading(_, c) => collect_links(c, out),
            _ => {}
        }
    }
}

#[test]
fn roundtrip_dictionary_strong_keys_unicode_links_and_library_scan() {
    let dir = tempdir().unwrap();
    let docx_path = dir.path().join("strong_sintetico.docx");
    let out_twm = dir.path().join("strong_sintetico.dct.twm");

    let body = r#"
      <w:p><w:pPr><w:pStyle w:val="Ttulo1"/></w:pPr><w:r><w:t>Grego e Hebraico</w:t></w:r></w:p>
      <w:p><w:pPr><w:pStyle w:val="Ttulo2"/></w:pPr>
        <w:r><w:t>[[@GreekStrongs:G25]][[@louw:25.43]][[@GreekGK:G26]] G25</w:t></w:r>
      </w:p>
      <w:p>
        <w:r><w:t>[[@Headword+el:ἀγαπάω]]</w:t></w:r>
        <w:r><w:rPr><w:b/><w:i/></w:rPr><w:t>{{field-on:gloss}}amar com devoção</w:t></w:r>
        <w:r><w:t>{{field-off:gloss}} — ver [[HebrewStrongs: H24]] e destino ausente [[GreekStrongs: G9999]].</w:t></w:r>
      </w:p>
      <w:p><w:pPr><w:pStyle w:val="Ttulo2"/></w:pPr>
        <w:r><w:t>[[@HebrewStrongs:H0024]][[@Headword+he:אָבִיב]][[@TWOT:1]] H24</w:t></w:r>
      </w:p>
      <w:p><w:r><w:t>Espiga nova em hebraico: בְּרֵאשִׁית e acentos çãéõ. Veja [[GreekStrongs: G25]].</w:t></w:r></w:p>
    "#;
    let doc = wrap_doc(body);
    write_zip(
        &docx_path,
        &[
            ("word/styles.xml", STYLES_XML.as_bytes(), true),
            ("word/document.xml", doc.as_bytes(), true),
        ],
    );

    let imported = import_docx(&docx_path, &ImportOptions::new(ImportKind::Dict)).unwrap();
    let opts = TwmExportOptions::new(
        out_twm.clone(),
        "Léxico Sintético".into(),
        "Descrição sintética".into(),
    );
    let exp = write_twm(ImportKind::Dict, &imported.entries, &opts, None).unwrap();

    assert_eq!(exp.ir_entries, 2);
    assert_eq!(exp.written_entries, 2);
    assert_eq!(exp.internal_links_resolved, 2);
    assert_eq!(exp.internal_links_unresolved, 1);

    // Recusa sobrescrita sem --force
    let err = write_twm(ImportKind::Dict, &imported.entries, &opts, None).expect_err("recusar");
    assert!(matches!(err, ImportError::OutputExists(_)));

    // Sobrescreve com force=true
    let mut opts_force = opts.clone();
    opts_force.force = true;
    write_twm(ImportKind::Dict, &imported.entries, &opts_force, None).expect("sobrescrever");

    // Nunca marcado como protegido
    ensure_twm_unprotected(&out_twm).expect("não protegido");
    let twm = TwmModule::open(&out_twm).expect("abrir TwmModule");
    assert_eq!(twm.config().get("type").map(String::as_str), Some("1"));
    assert_eq!(
        twm.config().get("content.type").map(String::as_str),
        Some("rtf")
    );
    assert_eq!(twm.config().get("user").map(String::as_str), Some("0"));
    assert!(twm.config().get("do.encrypt").is_none());
    assert!(twm.config().get("secure").is_none());

    // Round-trip via DictionarySource
    let dict = DictionarySource::open(&out_twm).expect("abrir DictionarySource");
    let g25 = dict.lookup("G25").unwrap().expect("G25");
    assert_eq!(g25.subject, "G25");
    assert_eq!(g25.strong_orig_word.as_deref(), Some("ἀγαπάω"));
    let g25_nodes = g25.content.expect("RTF G25 válido");
    let g25_text = plain_text(&g25_nodes);
    assert!(g25_text.contains("amar com devoção"), "{g25_text}");
    assert!(g25_text.contains("G9999"), "{g25_text}");
    let mut g25_links = Vec::new();
    collect_links(&g25_nodes, &mut g25_links);
    assert!(
        g25_links.contains(&LinkTarget::Dictionary("H24".into())),
        "esperado link interno H24: {g25_links:?}"
    );
    assert!(
        !g25_links.contains(&LinkTarget::Dictionary("G9999".into())),
        "destino inexistente não deve virar link"
    );

    let h24 = dict.lookup("H24").unwrap().expect("H24");
    assert_eq!(h24.strong_orig_word.as_deref(), Some("אָבִיב"));
    let h24_nodes = h24.content.expect("RTF H24 válido");
    let h24_text = plain_text(&h24_nodes);
    assert!(h24_text.contains("בְּרֵאשִׁית"), "{h24_text}");
    assert!(h24_text.contains("çãéõ"), "{h24_text}");

    // Tabela extra rhema_keys
    let conn = Connection::open_with_flags(&out_twm, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let key_count: i64 = conn
        .query_row("SELECT count(*) FROM rhema_keys", [], |r| r.get(0))
        .unwrap();
    assert!(key_count >= 6, "rhema_keys={key_count}");
    let has_content_orig: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'content_orig'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(has_content_orig, 0);

    // Library::scan classifica como Pronto
    let cache_dir = tempdir().unwrap();
    let lib = Library::new(cache_dir.path());
    let scan = lib.scan(&[dir.path().to_path_buf()], |_| {}).unwrap();
    let found = scan
        .modules
        .iter()
        .find(|m| m.path == out_twm)
        .expect("módulo catalogado");
    assert_eq!(found.kind, ModuleKind::Dictionary);
    assert_eq!(found.state, ModuleState::Ready);
    assert!(found.has_strong);
}

#[test]
fn roundtrip_commentary_bible_refs_autolinks_images_and_untagged_skip() {
    let dir = tempdir().unwrap();
    let docx_path = dir.path().join("comm_sintetico.docx");
    let out_twm = dir.path().join("comm_sintetico.cmt.twm");

    let body = r#"
      <w:p><w:pPr><w:pStyle w:val="Ttulo1"/></w:pPr><w:r><w:t>João</w:t></w:r></w:p>
      <w:p><w:pPr><w:pStyle w:val="Ttulo2"/></w:pPr><w:r><w:t>[[@Bible: Jo 3.16-18]] Novo Nascimento</w:t></w:r></w:p>
      <w:p>
        <w:r><w:t>[[@Bible: Jo 3.16]] Texto sobre amor (Gn 31.28) e </w:t></w:r>
        <w:hyperlink r:id="rIdBib1"><w:r><w:t>Rm 5.8</w:t></w:r></w:hyperlink>
      </w:p>
      <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Ponto um</w:t></w:r></w:p>
      <w:p><w:r><w:drawing><wp:docPr id="1" name="Img"/><a:blip r:embed="rIdImg1"/></w:drawing></w:r></w:p>
      <w:tbl><w:tr><w:tc><w:p><w:r><w:t>Col A</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Col B</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
      <w:p><w:pPr><w:pStyle w:val="Ttulo2"/></w:pPr><w:r><w:t>Seção sem passagem bíblica</w:t></w:r></w:p>
      <w:p><w:r><w:t>Não deve ser gravada em bible_refs.</w:t></w:r></w:p>
    "#;
    let doc = wrap_doc(body);
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdBib1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="tw://bible.*?id=45.5.8|_AUTODETECT_|" TargetMode="External"/>
  <Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/m1.png"/>
</Relationships>"#;
    let png = [0x89, b'P', b'N', b'G', 1, 2, 3, 4];
    write_zip(
        &docx_path,
        &[
            ("word/styles.xml", STYLES_XML.as_bytes(), true),
            ("word/_rels/document.xml.rels", rels.as_bytes(), true),
            ("word/media/m1.png", &png, false),
            ("word/document.xml", doc.as_bytes(), true),
        ],
    );

    let mut import_opts = ImportOptions::new(ImportKind::Comm);
    import_opts.collect_problems = true;
    let mut imported = import_docx(&docx_path, &import_opts).unwrap();
    let opts = TwmExportOptions::new(
        out_twm.clone(),
        "Comentário Sintético".into(),
        "Notas sintéticas".into(),
    );
    let exp = write_twm(
        ImportKind::Comm,
        &imported.entries,
        &opts,
        Some(&mut imported.problems),
    )
    .unwrap();

    assert_eq!(exp.ir_entries, 2);
    assert_eq!(exp.written_entries, 1);
    assert_eq!(exp.skipped_without_passage, 1);
    assert_eq!(exp.images_omitted, 1);
    assert_eq!(exp.bible_links_written, 2); // Gn 31.28 (auto) + Rm 5.8 (hyperlink)
    assert!(imported
        .problems
        .iter()
        .any(|p| p.detail.as_deref() == Some("sem_passagem")));

    // Round-trip via CommentarySource
    let comm = CommentarySource::open(&out_twm).expect("abrir CommentarySource");
    let entries = comm
        .entries_for(VerseRef::new(Book::John, 3, 16))
        .expect("consultar Jo 3:16");
    assert_eq!(entries.len(), 1);
    assert_eq!(
        (entries[0].bi, entries[0].ci, entries[0].fvi, entries[0].tvi),
        (43, 3, 16, 18)
    );
    let nodes = entries[0].content.as_ref().expect("RTF comentário válido");
    let mut links = Vec::new();
    collect_links(nodes, &mut links);
    let bible_links = links
        .iter()
        .filter(|t| matches!(t, LinkTarget::Bible(_)))
        .count();
    assert_eq!(bible_links, 2, "links={links:?}");

    // Library::scan classifica como Pronto
    let cache_dir = tempdir().unwrap();
    let lib = Library::new(cache_dir.path());
    let scan = lib.scan(&[dir.path().to_path_buf()], |_| {}).unwrap();
    let found = scan
        .modules
        .iter()
        .find(|m| m.path == out_twm)
        .expect("comentário catalogado");
    assert_eq!(found.kind, ModuleKind::Commentary);
    assert_eq!(found.state, ModuleState::Ready);
}
