# 03 — Arquitetura

## 1. Visão geral

```
┌──────────────────────────── Tauri 2 ────────────────────────────┐
│  UI (Svelte + Tailwind)  ◄── IPC (commands/events, JSON) ──►  src-tauri  │
└───────────────────────────────────────────────────┬──────────────┘
                                                    │ chama
        ┌───────────────┬────────────────┬──────────┴───┬────────────────┐
    rhema-library   rhema-search    rhema-sermon    rhema-refs      rhema-twformat
    (catálogo)       (tantivy)       (sermões)       (referências)   (.ont / .twm)
        └───────────────┴────────────────┴──────────────┴────────────────┘
                                  rhema-core (tipos)
```

Regra de dependência: **crates de baixo nunca conhecem Tauri.** Todo o núcleo é biblioteca pura, testável sem interface. Isso permite CLI, testes automáticos e portar para outras plataformas no futuro.

## 2. Responsabilidades dos crates

| Crate | Responsabilidade | Dependências-chave |
|---|---|---|
| `rhema-core` | `Book`, `VerseRef`, `VerseRange`, tabela canônica, `Node`, `ModuleInfo`, erros | `serde`, `thiserror` |
| `rhema-refs` | Parser/formatador de referências pt-BR e en (`Jo 3:16-18`, `1Co 13`, `Sl 23:1-6; 27:1`) | — |
| `rhema-twformat` | Leitores `.ont/.ot/.nt` e `.twm`; tokenizador de marcação | `memmap2`, `rusqlite` |
| `rhema-library` | Descobrir pastas, classificar módulos, cache do catálogo, abrir fontes sob demanda | `walkdir`, `notify` |
| `rhema-search` | Índice tantivy por módulo, busca por frase/palavra e por Strong | `tantivy` |
| `rhema-sermon` | Modelo de sermão, autosave atômico, realces/anotações, exportações | `rusqlite`, `printpdf`/`docx-rs` |
| `rhema-cli` | `inspect`, `dump`, `search`, `bench` | `clap` |

## 3. Contrato IPC (Tauri commands)

Todos assíncronos, retornam `Result<T, ApiError>` serializável. Nomes em `snake_case`.

| Comando | Entrada | Saída |
|---|---|---|
| `library_scan` | `paths?` | `Vec<ModuleInfo>` (+ evento `library:progress`) |
| `library_list` | — | `Vec<ModuleInfo>` |
| `resolve_reference` | `text` | `Vec<VerseRange>` ou sugestões |
| `get_chapter` | `module_id, book, chapter` | `Vec<VerseDto>` (nós já prontos para renderizar) |
| `get_chapters` | `module_ids[] (≤ 4), book, chapter` | linhas alinhadas por número de versículo, com nós e erro por coluna |
| `lookup_strong` | `strong_id` | entrada do léxico (título, `original_word`, nós, módulo de origem); erros `no_lexicon_installed`, `entry_not_found`, `unsupported_format`, `invalid_strong_id` |
| `get_commentary_index` | `book, chapter, verse` | por comentário ativo, cabeçalhos (escopo, rótulo, tamanho, `supported`), do mais específico ao mais amplo; sem conteúdo |
| `get_commentary_entry` | `module_id, topic_id, offset` | página de até 60 parágrafos (`nodes`, `has_more`, `next_offset`); erros `unsupported_format` (tipo desconhecido), `entry_not_found`, `module_protected`; resposta com `degraded` |
| `get_xref_preview` | `range, module_id` | texto curto para o cartão |
| `search` | `query, scope` | resultados paginados + trechos realçados |
| `sermon_*` | create/open/save/list/export/delete | modelo de sermão |
| `annotation_*` | add/list/remove | realces e notas por versículo |
| `projection_*` | open/close/show(range\|slide) | controle da janela de projeção |
| `settings_*` | get/set | preferências |

Eventos: `library:progress`, `index:progress`, `sermon:saved`, `projection:state`.

**Segurança:** CSP restritiva; sem `allow-all` no Tauri; capabilities mínimas; HTML vindo de módulo **nunca** é injetado diretamente: a UI recebe `Node` estruturado e renderiza componentes.

## 4. Dados do usuário

- Pasta de dados (padrão do SO): `rhema.db` (SQLite WAL), `indexes/`, `catalog/`, `exports/`.
- `rhema.db`: tabelas `sermons`, `sermon_blocks`, `annotations`, `highlights`, `settings`, `recent`. Migrações versionadas (`refinery` ou `sqlx migrate`).
- Sermão = lista ordenada de **blocos tipados** (`heading`, `text`, `verse_ref`, `lexicon`, `note`, `illustration`). Versículo inserido guarda **a referência e a tradução**, não só o texto, para atualizar se o usuário trocar de tradução.
- Todo sermão é exportável para **Markdown** com front-matter, garantindo que o conteúdo nunca fique preso ao app.
- Autosave: debounce 500 ms; escrita transacional; cópia de segurança rotativa (últimas 20 versões por sermão).

## 5. Busca

- Um índice tantivy por módulo de Bíblia, criado em segundo plano na primeira abertura, guardado em `indexes/<module_id>/`.
- Campos: referência, texto limpo (sem tags), Strong ids. Tokenização com remoção de acentos e minúsculas (pt-BR).
- Busca por Strong consulta índice invertido dedicado.
- A interface mostra progresso de indexação sem bloquear leitura.

## 6. Orçamentos de desempenho (requisito)

| Medida | Orçamento |
|---|---|
| Abrir o app até tela utilizável | < 1 s (máquina modesta, SSD) |
| Abrir um capítulo | < 30 ms |
| Abrir módulo `.ont` pela primeira vez | < 200 ms |
| Busca simples em uma Bíblia (índice pronto) | < 100 ms |
| RAM com 10 módulos abertos | < 80 MB no processo Rust |
| Instalador | < 25 MB |

Benchmarks `criterion` no CI; regressão > 20% falha a build.

## 7. Frontend

- Estado: *stores* do Svelte; nenhuma lógica de formato no frontend.
- Componentes-chave: `CommandBar`, `LibraryTree`, `BiblePane` (virtualizado), `ParallelColumns`, `StrongPopover`, `SidePanel` (Léxico/Comentário/Referências), `SermonEditor`, `PulpitView`, `ProjectionView`.
- Estilo: tokens Tailwind do protótipo (ver `docs/01-produto-ux.md` §7). Tema claro/escuro por variável CSS.
- Todo texto vem de `i18n/pt-BR.json` e `i18n/en.json`.
- Testes: Playwright cobrindo fluxos críticos (abrir passagem, clicar Strong, inserir versículo no sermão, entrar no Modo Púlpito).

## 8. Janelas

- **Principal:** estudo e edição.
- **Projeção:** janela sem bordas, escolhe monitor, comunica por eventos; recebe apenas dados de exibição.
- Modo Púlpito é uma *view* da janela principal (tela cheia opcional).

## 9. Empacotamento e releases

- CI (GitHub Actions): `fmt`, `clippy`, `test`, `bench`, build Tauri em Windows/Linux/macOS.
- Artefatos: MSI (Windows), dmg (macOS), AppImage/deb (Linux).
- Versionamento semântico; `CHANGELOG.md` mantido por milestone.
- Atualização automática **opcional**, desligada por padrão.

## 10. Observabilidade local

- Log em arquivo rotativo (`tracing`), nível configurável; **sem telemetria** externa.
- Botão "Copiar diagnóstico" gera relatório (versão, SO, módulos reconhecidos, erros recentes) para suporte, sem conteúdo dos sermões.
