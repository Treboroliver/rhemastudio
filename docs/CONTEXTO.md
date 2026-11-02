# Contexto — M6a

Atualizado em 2026-11-02. M6a concluído (backend do sermão, sem interface); M4b continua fora do escopo. Pendências conhecidas: `docs/PENDENCIAS.md`.

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
