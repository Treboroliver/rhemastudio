# Changelog — RhemaStudio

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
