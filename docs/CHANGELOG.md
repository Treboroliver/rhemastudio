# Changelog — RhemaStudio

## [M1.3] — 2026-10-24

### Concluído
- Tokenizador tolerante de tags bíblicas para `Vec<Node>`, com suporte às tags confirmadas, preservação de tags desconhecidas e fechamento seguro de marcação desbalanceada.
- Strong associado à última palavra contígua anterior (pontuação adjacente incluída); Strong consecutivos compartilham o trecho por aninhamento.
- `plain_text`, `OntReader::verse_nodes` e `chapter_nodes`; opções `rhema-cli dump --parsed` e `--plain`.
- Cobertura unitária e `proptest` de preservação textual, snapshots de versículos sintéticos e teste opcional de todos os versículos reais via `RHEMA_TEST_MODULES`.
- Varredura release do `kjv.ont`: 31.102 versículos, zero `UnknownTag`, texto simples sem `<`/`>`; tokenização total em 463,9 ms. Confirmada a variante real `<WG####x>`.
- Tags `<TS>`, `<CL>`, `<WT…>` e `<RX…>` permanecem marcadas **[VERIFICAR]** em `docs/02-formato-theword.md`.

## [M1.2] — 2026-10-18

### Concluído
- Implementação de `OntReader` em `rhema-twformat` para leitura de alta performance de Bíblias em texto plano (`.ont`, `.ot`, `.nt`):
  - Mapeamento em memória com `memmap2` e indexação vetorial de linhas via SIMD (`memchr`), sem carregar o arquivo em `String`.
  - Detecção rigorosa de assinatura binária de DRM (`TWENCBMOD`) retornando o erro tipado `ModuleError::EncryptedModule`.
  - Delimitação robusta entre versículos e bloco de propriedades ao final do arquivo, suportando comentários (`#`), linhas de continuação de texto e propriedades `chave=valor`.
  - Resolução de codificação baseada na propriedade `charset` (`0` para Windows-1252, `65001` para UTF-8) com fallback seguro.
  - Charset ausente ou desconhecido agora valida UTF-8 antes do fallback Windows-1252 e registra o charset selecionado; coberto por fixtures sintéticas.
  - Suporte completo aos escopos `Full` (31.102 versículos), `OldTestament` (23.145 versículos) e `NewTestament` (7.957 versículos), com abertura em modo degradado para módulos de contagem não canônica (sem pânico).
  - APIs de leitura `info()`, `verse(book, chapter, verse)`, `chapter(book, chapter)` e `line_by_index(idx)`.
- Novo comando `rhema-cli dump <arquivo> --book <N> --chapter <C> --verse <V>` para exibição de versículos brutos com tags.
- Suíte abrangente de testes:
  - Testes unitários com geradores sintéticos em `tools/gen-fixtures` cobrindo Bíblias completas, AT, NT, módulos degradados e arquivos truncados/vazios.
  - Testes de propriedade com `proptest` para resiliência a bytes arbitrários e contagens aleatórias de linhas.
  - Benchmark release com módulo real `kjv.ont`: primeira abertura medida em 65,3 ms (cache frio aproximado), reabertura em 7,4 ms (cache quente) e leitura de João 3 em 70,9 µs. O orçamento de 200 ms aplica-se à abertura com cache quente; o sistema operacional não permite garantir cache frio neste teste.

---

## [M1.1] — 2026-10-18

### Concluído
- Implementação da ferramenta `rhema-cli inspect <arquivo>` para inspecionar com segurança (estritamente somente leitura):
  - Bancos SQLite (`.twm`, `.mybible`): DDL via `sqlite_master`, metadados (`config`), análise de referências bíblicas (`bible_refs`), análise de chaves Strong (`topics`), contagem e amostras de linhas.
  - Bíblias em texto plano (`.ont`, `.ot`, `.nt`): detecção de codificação (UTF-8, Windows-1252), contagem exata de linhas de versículos, extração de bloco de propriedades `chave=valor` e distribuição completa de famílias de tags XML/theWord.
  - Módulos encriptados com DRM (`.ontx`, `.twmx`): detecção da assinatura binária `TWENCBMOD` com tratamento e mensagem informativa clara ao usuário.
- Inspeção e validação de módulos theWord reais:
  - `kjv.ont` (7,7 MB): 31.102 versículos (exatamente idêntico à contagem canônica), 39 propriedades, 485.927 tags analisadas.
  - `Mickelson's Strong.dct.twm` (12,6 MB): chaves Strong `G{n}` e `H{n}` sem zeros à esquerda, tabela `topics` com coluna `strong_orig_word` (Unicode).
  - `Holman-AT.cmt.twm` (126,9 MB) e `NICNT.cmt.twm` (93,6 MB): tabela `bible_refs` (`bi, ci, fvi, tvi`) mapeando introduções de livros (`ci=0`), introduções de capítulos (`fvi=0`), versículos individuais (`fvi=tvi`) e perícope/faixas (`fvi<tvi`).
  - `acf2007.ontx` (1,4 MB): confirmação do cabeçalho binário `TWENCBMOD`.
- Criação de `docs/02-apendice-schemas.md` com os DDLs reais, exemplos e queries SQL otimizadas.
- Atualização de `docs/02-formato-theword.md` e fechamento de todos os itens de verificação técnica da M1.1.

---

## [M0] — 2026-10-18

### Concluído
- Criação do workspace Cargo com os 7 crates: `rhema-core`, `rhema-refs`, `rhema-twformat`, `rhema-library`, `rhema-search`, `rhema-sermon`, `rhema-cli`.
- `rhema-core`: tipos fundamentais `Book` (66 livros canônicos, nomes e abreviações pt-BR e en), `VerseRef`, `VerseRange`, `ModuleInfo`, erros tipados e tabela canônica verificada de 31.102 versículos (23.145 AT, 7.957 NT).
- Aplicação desktop Tauri 2 com frontend Svelte 5 + Tailwind CSS e tema escuro.
- Configuração de CI GitHub Actions (`fmt`, `clippy -D warnings`, `test`).
- Estruturação e migração da documentação (`AGENTS.md` na raiz, `docs/01-produto-ux.md`, `docs/02-formato-theword.md`, `docs/03-arquitetura.md`, `docs/04-roadmap.md`).
