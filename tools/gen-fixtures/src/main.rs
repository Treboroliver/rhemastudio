use anyhow::{bail, Result};
use rhema_core::{TOTAL_CANONICAL_BOOKS, TOTAL_CANONICAL_VERSES};
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    println!("RhemaStudio — Ferramenta de Geração de Fixtures Sintéticas");
    println!(
        "Cânon bíblico configurado: {} livros, {} versículos",
        TOTAL_CANONICAL_BOOKS, TOTAL_CANONICAL_VERSES
    );

    let fixtures_dir = Path::new("fixtures");
    if !fixtures_dir.exists() {
        fs::create_dir_all(fixtures_dir)?;
        println!("Criado diretório de fixtures: {}", fixtures_dir.display());
    }

    let readme_path = fixtures_dir.join("README.md");
    if !readme_path.exists() {
        fs::write(
            &readme_path,
            "# Fixtures de Teste do RhemaStudio\n\nEste diretório contém módulos sintéticos gerados para testes automatizados.\n",
        )?;
    }

    for filename in [
        "synthetic-commentary.cmt.twm",
        "synthetic-dictionary.dct.twm",
        "synthetic-encrypted.cmt.twm",
        "synthetic-secure.cmt.twm",
    ] {
        if fixtures_dir.join(filename).exists() {
            bail!(
                "Fixture sintética já existe: {}",
                fixtures_dir.join(filename).display()
            );
        }
    }
    gen_fixtures::generate_synthetic_commentary_twm(
        &fixtures_dir.join("synthetic-commentary.cmt.twm"),
    )?;
    gen_fixtures::generate_synthetic_dictionary_twm(
        &fixtures_dir.join("synthetic-dictionary.dct.twm"),
    )?;
    gen_fixtures::generate_synthetic_protected_twm(
        &fixtures_dir.join("synthetic-encrypted.cmt.twm"),
        "do.encrypt",
    )?;
    gen_fixtures::generate_synthetic_protected_twm(
        &fixtures_dir.join("synthetic-secure.cmt.twm"),
        "secure",
    )?;
    println!(
        "Quatro módulos .twm sintéticos gerados em {}.",
        fixtures_dir.display()
    );
    Ok(())
}
