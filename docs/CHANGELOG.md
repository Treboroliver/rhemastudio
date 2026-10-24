# Changelog — RhemaStudio

## [M1.3c] — 2026-10-24

### Concluído
- Investigação somente leitura do dicionário Mickelson encontrou `compressed=1`, `do.encrypt=1` e `secure=1`; essa evidência encerrou a análise sem decodificar nem descomprimir qualquer payload.
- Registrados no apêndice os indicadores de configuração conhecidos, tipos/tamanhos/entropia, prefixos de bytes e pontos ainda sem resposta, sem reproduzir texto de módulo.
- Adicionado `ensure_twm_unprotected`, que abre SQLite estritamente em leitura e rejeita flags observadas de proteção com `ModuleError::EncryptedModule` antes de consultas de conteúdo; o comando `inspect` e a amostragem RTF passam pela verificação.
- Não foi implementado leitor/decodificador para o Mickelson. Algoritmo de compressão, dados auxiliares e destinos `tw://` permanecem sem esclarecimento; não foi iniciado M1.4.

## [M1.3b] — 2026-10-24

### Concluído
- Conversor iterativo e tolerante de RTF para `Vec<Node>` em `rhema-twformat`, com grupos aninhados, escapes `\'hh` conforme `\ansicpg`, `\uN`/`\ucN`, sequências binárias `\binN`, formatação, quebras, campos de hiperlink, destinos especiais e contagem de controles/destinos desconhecidos.
- `Node` ampliado com negrito, sublinhado, tachado, caixa alta, sobrescrito, subscrito e links tipados para Bíblia/dicionário/outros; `plain_text` segue removendo notas e metadados estruturais.
- Novo `rhema-cli rtf-sample <arquivo.twm> --count N`: amostragem uniforme em modo SQLite somente leitura, grava texto em diretório temporário e limita a três prefixos hex para BLOBs.
- Cobertura sintética unitária, snapshots `insta` e `proptest` com bytes arbitrários; teste opcional lê 2.000 linhas distribuídas de cada arquivo quando `RHEMA_TEST_MODULES` está configurada.
- Amostra release: Mickelson 2.000/2.000 BLOBs não RTF (0,025 ms/entrada para classificar); Holman 0 controles/destinos desconhecidos, 0 texto residual com `\`/`{` (0,746 ms/entrada); NICNT 0 desconhecidos, 1 texto com `\` e 3 com chaves, correspondendo a escapes literais na origem (1,262 ms/entrada).
- Links bíblicos identificados na amostra release: 2.164 Holman e 59.658 NICNT; destinos `tw://` não classificados preservados como `Other` (25 e 1.239, respectivamente). Não foi encontrado link real de dicionário.
- Schemas, tipos de armazenamento, contagens de controles RTF, codificação e limites dos BLOBs registrados em `docs/02-apendice-schemas.md`. Nenhum conteúdo de módulo real foi copiado para a documentação ou fixtures.

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
