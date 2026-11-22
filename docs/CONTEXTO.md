# Contexto — M6b-2

Atualizado em 2026-11-02. M6b-2 concluído (sermão ligado à leitura); M4b continua fora do escopo. Pendências conhecidas: `docs/PENDENCIAS.md`.

## Plano de exegese

Referências: [plano](06-plano-exegese.md) e [inventário E0](07-inventario-exegetico.md).

- E0: concluído — inventário local e CLI somente leitura.
- E0b: concluído — testes de mapeamento lexical e recomendação E1a em [08-mapeamento-lexical.md](08-mapeamento-lexical.md).
- E1a.1: concluído — núcleo lexical, extração e ligações locais; detalhes e limites em [09-indice-lexical.md](09-indice-lexical.md).
- E1a.2: concluído — índice de léxicos (tantivy), busca e CLI; detalhes em [09-indice-lexical.md](09-indice-lexical.md).
- E1a.3: concluído — decodificador HTML, desempate por tipo e total real; ver [09-indice-lexical.md](09-indice-lexical.md).
- E1b.1: concluído — índice reverso palavra → Strong e integração na busca; ver [09-indice-lexical.md](09-indice-lexical.md).
- E1b.2: concluído — busca léxica no app (`Ctrl+K`, aba Léxico e índices em segundo plano); ver [09-indice-lexical.md](09-indice-lexical.md).
- E1b.3: concluído — abertura por `topic_id`, títulos limpos (`clean_title`), mensagens específicas e remoção de pastas; ver [09-indice-lexical.md](09-indice-lexical.md).
- E1b.4: concluído — correção de abertura dos dicionários comuns (`twm.rs`/`rhema-library`), lemas/refs em genéricos (`v3`) e menu `Outros dicionários (N)` (`lex_related`); ver [09-indice-lexical.md](09-indice-lexical.md).
- E1b.5: concluído — links RTF inline (`\fldrslt`), descompressão zlib (`compressed=1`, teto de 64 MB `content_too_large`), classificação por `config.type`, resolução `meta`, `topics_flags.hidden=1` e erros de comentário com `lex-errors.log`; ver [09-indice-lexical.md](09-indice-lexical.md).
- A1.1: concluído — crate `rhema-import` (leitura `.docx` em fluxo, limites contra bomba de zip/entrada/imagem/tabela, gramática de marcadores, IR e relatório de validação) e CLI `rhema-cli import docx`; ver [11-importador-docx.md](11-importador-docx.md).
- A1.2: concluído — gravador `.twm` de dicionário e comentário a partir da IR (`--out`, RTF Tahoma 10 pt com `\uN?`, `rhema_keys`, links internos/bíblicos, `--problemas`); ver [11-importador-docx.md](11-importador-docx.md).
- A1.3: pendente — formato próprio apenas se o `.twm` não bastar, e `kind book` (`.gbk.twm`).
- E2a: pendente — painel de palavra.
- E2b: pendente — interlinear.
- E3: pendente — textos originais e morfologia.
- E4a: pendente — editor rico e persistência.
- E4b: pendente — nós bíblicos e cartões.
- E4c: pendente — recursos avançados do editor.
- E4d: pendente — exportação e impressão.
- E5.0: pendente — avaliação das bibliotecas de quadro.
- E5a: pendente — quadro persistente.
- E5b: pendente — diagramas e incorporação.
- M7: pendente — púlpito e projeção.
- M5, M4b, M8: pendentes — busca, referências cruzadas e acabamento.

- Sermão × leitura (M6b-2): comandos `get_verse_snapshot` (≤80 versículos, faixas numeradas, sem notas/títulos) e `get_lexicon_snapshot` (≤1.500 caracteres) em `app/src-tauri/src/snapshot.rs`. UI: `sermonSession.ts` (sermão ativo = último aberto; insere no editor aberto depois do último bloco focado, ou grava direto no banco), `SermonPicker` ("Em qual sermão?"), `VerseRefBox` (menu "/" → referência), aviso com Desfazer (8 s) no `App.svelte`, Ctrl+Enter global. Bloco Versículo: trocar tradução, ir para a leitura, módulo ausente mostra o texto salvo. O teste de 1.000 blocos só roda com `RHEMA_SLOW_TESTS=1`.
- UI do sermão (M6b-1): aba Sermão no painel direito (lista, editor de blocos, autosave 500 ms com fila, Ctrl+S, gravação ao voltar/trocar de aba/fechar, versões, exportação Markdown). Botão Expandir (420 px ↔ 50%, `study_panel_expanded`). Código em `app/ui/src/lib/Sermon*.svelte` e `sermon.ts`. Busca por título sem acentos no backend. Inserção de versículo/léxico: M6b-2.
- Sermão: crate `rhema-sermon` (modelo de blocos, SQLite WAL + `user_version` v2, versões rotativas: ≤1/min, 20 mais recentes, restore reversível, export Markdown). Comandos Tauri `sermon_*` em `app/src-tauri/src/sermon.rs`; banco `rhema.db` na pasta de dados do app; `sermon_save_export` usa o diálogo nativo. 1.000 blocos em release: salvar 16 ms, carregar 5 ms.

## M1.5.1 (anterior)

- RVF: cabeçalhos com campos extras, tag entre aspas e espaço final agora são reconhecidos; blobs de célula/tabela descartados; UTF-16 validado (>30% suspeitos → Windows-1252). 0 entradas suspeitas nos 4 módulos. Links: só `tw://bible` é de versículo; marcadores `-9 … 3/4` são estruturais.
- UI: `FootnoteMarker` não gera mais espaço ao redor da nota (`textContent` == `plain_text`, teste `node-spacing.test.ts`).
- RVF: `rvf.rs` converte as entradas de VidaNova (110), Strong_AMG2 (32, incl. G3056), BEP (1.181) para `Vec<Node>`; Holman tem 1 entrada RVF com dados NULL. Estrutura estranha vira `degraded` (aviso discreto na interface), nunca erro. Especificação em `docs/02-apendice-schemas.md`.
- RTF/espaços: a medição (2.000 entradas) mostrou que o conversor não descarta espaços reais; o "espaço perdido" é o delimitador após palavra de controle (RTF). Sem alteração no conversor.
- Biblioteca: valores de propriedade têm espaços e o sufixo ` <-` removidos (o marcador está no arquivo da BJ, em `lang` e `description`); `CATALOG_VERSION` = 5.
- Abertura `.twm` consulta só chaves selecionadas de `config`, nunca `about`.
- Comentários seguem paginados por até 60 parágrafos e 8.000 caracteres.
- Restrições observadas: conteúdo de módulos protegidos não foi aberto; nenhum texto real de módulo foi salvo em código, testes ou documentos.
