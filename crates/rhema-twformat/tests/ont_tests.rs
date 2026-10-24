use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use proptest::prelude::*;
use rhema_core::{Book, ModuleError};
use rhema_twformat::{BibleScope, OntReader};
use tempfile::NamedTempFile;

#[test]
fn test_synthetic_ont_full_lifecycle() {
    let temp = NamedTempFile::new().unwrap();
    gen_fixtures::generate_synthetic_ont(temp.path(), 0, true).unwrap();

    let reader = OntReader::open(temp.path()).unwrap();
    assert_eq!(reader.scope(), BibleScope::Full);
    assert!(!reader.is_degraded());
    assert_eq!(reader.verse_count(), 31102);

    // Gênesis 1:1 (início AT)
    let v_gen = reader.verse(Book::Genesis, 1, 1).unwrap();
    assert!(v_gen.contains("Gn 1:1"));
    assert!(v_gen.contains("<WH3068>"));

    // Malaquias 4:6 (fim AT)
    let v_mal = reader.verse(Book::Malachi, 4, 6).unwrap();
    assert!(v_mal.contains("Ml 4:6"));

    // Mateus 1:1 (início NT)
    let v_mat = reader.verse(Book::Matthew, 1, 1).unwrap();
    assert!(v_mat.contains("Mt 1:1"));

    // João 3:16 (meio NT)
    let v_john = reader.verse(Book::John, 3, 16).unwrap();
    assert!(v_john.contains("Jo 3:16"));
    assert!(v_john.contains("<FR>"));

    // Apocalipse 22:21 (fim NT)
    let v_rev = reader.verse(Book::Revelation, 22, 21).unwrap();
    assert!(v_rev.contains("Ap 22:21"));

    // Leitura completa de capítulo
    let john_3 = reader.chapter(Book::John, 3).unwrap();
    assert_eq!(john_3.len(), 36);
}

#[test]
fn test_robustness_short_and_truncated_files() {
    // 1 byte
    let mut f1 = NamedTempFile::new().unwrap();
    f1.write_all(b"A").unwrap();
    f1.flush().unwrap();
    let r1 = OntReader::open(f1.path()).unwrap();
    assert!(r1.is_degraded());

    // 8 bytes (abaixo do tamanho da assinatura TWENCBMOD)
    let mut f8 = NamedTempFile::new().unwrap();
    f8.write_all(b"12345678").unwrap();
    f8.flush().unwrap();
    let r8 = OntReader::open(f8.path()).unwrap();
    assert!(r8.is_degraded());

    // Linhas truncadas sem newline final
    let mut f_trunc = NamedTempFile::new().unwrap();
    f_trunc
        .write_all(b"Linha 1\r\nLinha 2 sem quebra final")
        .unwrap();
    f_trunc.flush().unwrap();
    let r_trunc = OntReader::open(f_trunc.path()).unwrap();
    assert_eq!(r_trunc.verse_count(), 2);
    assert_eq!(r_trunc.line_by_index(0).unwrap(), "Linha 1");
    assert_eq!(
        r_trunc.line_by_index(1).unwrap(),
        "Linha 2 sem quebra final"
    );
}

#[test]
fn test_real_modules_integration_and_performance() {
    // Testes com módulos reais usam exclusivamente o diretório informado pelo usuário.
    let test_dir_env = match std::env::var("RHEMA_TEST_MODULES") {
        Ok(val) => val,
        Err(_) => {
            println!(
                "RHEMA_TEST_MODULES não definida. Pulando teste de integração com módulos reais."
            );
            return;
        }
    };

    let test_dir = PathBuf::from(test_dir_env);
    if !test_dir.exists() {
        println!(
            "Diretório RHEMA_TEST_MODULES ({}) não existe. Pulando.",
            test_dir.display()
        );
        return;
    }

    let kjv_path = test_dir.join("kjv.ont");
    if kjv_path.exists() {
        let path = &kjv_path;
        println!("Testando com módulo real: {}", path.display());

        // A primeira abertura representa a leitura inicial; o cache do SO não é controlável aqui.
        let t0 = Instant::now();
        let reader = OntReader::open(path).expect("Falha ao abrir kjv.ont real");
        let cold_open_duration = t0.elapsed();
        println!(
            "Primeira abertura do kjv.ont (cache frio aproximado): {:?}",
            cold_open_duration
        );

        let t1 = Instant::now();
        let warm_reader = OntReader::open(path).expect("Falha ao reabrir kjv.ont real");
        let warm_open_duration = t1.elapsed();
        println!(
            "Reabertura do kjv.ont (cache quente): {:?}",
            warm_open_duration
        );
        assert!(
            warm_open_duration.as_millis() < 200,
            "Orçamento de cache quente estourado: abertura demorou {:?}",
            warm_open_duration
        );

        // Verificações do kjv.ont
        assert_eq!(reader.scope(), BibleScope::Full);
        assert!(!reader.is_degraded());
        assert_eq!(reader.verse_count(), 31102);
        assert_eq!(warm_reader.verse_count(), 31102);

        // Benchmark de leitura de capítulo (João 3): Orçamento < 30 ms
        let t2 = Instant::now();
        let ch3 = reader.chapter(Book::John, 3).expect("Falha ao ler João 3");
        let chapter_duration = t2.elapsed();
        println!(
            "Tempo de leitura do capítulo João 3: {:?}",
            chapter_duration
        );
        assert!(
            chapter_duration.as_millis() < 30,
            "Orçamento estourado: leitura do capítulo demorou {:?}",
            chapter_duration
        );

        assert_eq!(ch3.len(), 36);

        // Verificar João 3:16 (índice 15)
        let j316 = &ch3[15].1;
        assert!(
            j316.contains("<FR>"),
            "João 3:16 deve conter palavras de Jesus em vermelho"
        );
        assert!(
            j316.contains("<WG"),
            "João 3:16 no KJV deve conter tags de Strong grego"
        );
    }

    let encrypted_path = test_dir.join("acf2007.ontx");
    if encrypted_path.exists() {
        println!(
            "Testando recusa de módulo encriptado: {}",
            encrypted_path.display()
        );
        let res = OntReader::open(encrypted_path);
        assert!(
            matches!(res, Err(ModuleError::EncryptedModule)),
            "Módulo encriptado deve retornar ModuleError::EncryptedModule"
        );
    }
}

// Testes de Propriedade (proptest)
proptest! {
    #[test]
    fn test_proptest_arbitrary_bytes_no_panic(bytes in prop::collection::vec(any::<u8>(), 0..5000)) {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&bytes).unwrap();
        temp.flush().unwrap();

        // O leitor NUNCA deve entrar em pânico com qualquer entrada de bytes aleatória
        let _ = OntReader::open(temp.path());
    }

    #[test]
    fn test_proptest_arbitrary_line_counts(lines in prop::collection::vec("[a-zA-Z0-9 <>,.]{1,80}", 1..200)) {
        let mut temp = NamedTempFile::new().unwrap();
        for l in &lines {
            writeln!(temp, "{}", l).unwrap();
        }
        writeln!(temp, "\r\ncharset=0\r\ntitle=PropTest Bible").unwrap();
        temp.flush().unwrap();

        let reader = OntReader::open(temp.path()).unwrap();
        prop_assert!(reader.verse_count() >= 1);
        prop_assert!(reader.is_degraded()); // contagem diferente de 31102/23145/7957
    }
}
