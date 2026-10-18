use anyhow::Result;
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

    // Criar arquivo placeholder inicial de fixtures
    let readme_path = fixtures_dir.join("README.md");
    if !readme_path.exists() {
        fs::write(
            &readme_path,
            "# Fixtures de Teste do RhemaStudio\n\nEste diretório contém módulos sintéticos gerados para testes automatizados.\n",
        )?;
        println!("Criado: {}", readme_path.display());
    }

    println!("Esqueleto de fixtures pronto. Geração completa de módulos sintéticos será ativada no M1.5.");
    Ok(())
}
