# RhemaStudio

Software de estudo bíblico e preparação de sermões para pastores e pregadores, desenvolvido em Rust e Tauri.

Compatível com módulos do theWord (`.ont`, `.ot`, `.nt` e `.twm`).

## Estrutura do Workspace

- `crates/rhema-core`: Modelos de domínio (`Book`, `VerseRef`, `VerseRange`, cânon bíblico, tipos de nós ricos).
- `crates/rhema-refs`: Parser e formatador de referências bíblicas (pt-BR e en).
- `crates/rhema-twformat`: Leitores para os formatos theWord (`.ont`, `.ot`, `.nt`, `.twm`).
- `crates/rhema-library`: Descoberta, catalogação e gerenciamento de módulos.
- `crates/rhema-search`: Mecanismo de busca e indexação textual e por Strong.
- `crates/rhema-sermon`: Gerenciamento, autosave e exportação de sermões e anotações.
- `crates/rhema-cli`: Ferramenta de linha de comando para inspeção e testes de módulos.
- `app/`: Aplicação desktop Tauri 2 com interface Svelte e Tailwind.
- `tools/gen-fixtures`: Utilitário para geração de fixtures sintéticas para testes.

## Comandos Principais

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
