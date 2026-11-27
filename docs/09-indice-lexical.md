# 09 — Índice lexical e exegese (E1b–E3)

> Histórico detalhado de E1a.1–E1a.3 em [09b-historico-lexical.md](09b-historico-lexical.md).

## E1b.1 — Índice reverso palavra → Strong
- **Estrutura (`lexicon-reverse.sqlite`):** indexa Bíblias legíveis com tags `Node::Strong` gravando só contagens `(n_ws, n_w, n_s)`, sem texto bíblico; tokenização `fold_latin` (`'`/`-` internos, sem números, `<2` letras); troca atômica e invalidação por tamanho/data.
- **Dice e medições (release):** `Dice = 2*n_ws/(n_w+n_s)` somando os módulos; descarta `n_ws < 2` (salvo acerto único). Build 5.514 ms, 13,3 MB, 5 protegidas ignoradas; abertura 3,31 ms, p50 0,36 ms.

## E1b.2 — Busca léxica no app
- `ReverseIndex` mantido aberto em `LexiconIndex` (`p95 = 6,59 ms`); comandos `lex_index_status`, `lex_index_build` (`spawn_blocking`, evento `lex-index-progress`), `lex_search` e `lex_entry`; interface `Ctrl+K` + aba Léxico com debounce de 120 ms e sugestões G/H lado a lado.


## E1b.3 — Abertura por topic_id e títulos limpos

- `INDEX_SCHEMA_VERSION = 2` guarda `topic_id` e aplica `clean_title`; `lookup_by_id` abre o verbete sem `key.trim()`, e `library_scan` tolera pastas ausentes (`library:warnings` + `Remover`).

## E1b.4 — Abertura universal, lemas comuns e relacionados

- `DictionarySource::open` exige só `(id, subject)`, `.dct.twm` aceita `type=2/3`, e `unknown` lê como texto simples. Genéricos (`v3`) indexam lemas gregos/hebraicos/mistos e refs Strong; `lex_related` + `"Outros dicionários (N)"`.


## E1b.5 — Links inline, zlib, meta, ocultos e erros de comentário

- RTF `\field` isola `\fldrslt` sem `\par` interno; `twm.rs` descomprime zlib (`compressed=1`), segue `config.type` → extensão, resolve atalhos `meta` (≤5 saltos) e oculta `topics_flags.hidden=1`; erros de comentário gravam em `lex-errors.log`.


## E2a — Concordância por Strong

- `occurrences.sqlite` (`WITHOUT ROWID`, tabelas `occurrences`, `book_counts`, `translations`, `word_totals`, idiomas `por/grc/heb`, troca atômica na mesma varredura do reverso) + `strong_occurrences`/`strong_translations` (ranqueado por Dice, descartando palavras ≤2 letras e `n_ws < 2` salvo única) + seção recolhível `Ocorrências` na aba Léxico. Release (6 Bíblias, 2 protegidas): build 25.642 ms, 64,7 MB, abertura 2,62 ms, 200 consultas p50 0,69 ms / p95 1,18 ms.

## E3 — Grego e hebraico legíveis, RTL, fontes e morfologia
- **Codificação RTF (`rtf.rs`):** interpreta `\fonttbl` (`\fcharset` 161→1253, 177→1255, 162→1254, 204→1251, 238→1250, 186→1257, 163→1258; `\cpg` prioritário; Symbol preserva fallback `\ansicpg`/1252; `\uN`/`\ucN` prioritários). `Dicionário Bíblico de Strong` (5.758 entradas gregas + 8.074 hebraicas, incl. G25) passou de 0% para 100% com blocos grego/hebraico válidos.
- **RTL e fontes:** colunas hebraicas com `dir="rtl"` e alinhamento à direita sem desalinhar colunas paralelas; isolamento bidi por trecho (`<bdi dir="rtl">`) em verbetes e comentários; interruptor `Mostrar cantilação` (desligado por padrão, preserva vogais); seleção de fontes locais gregas/hebraicas nas configurações (`auto` usa pilha local sem downloads).
- **Morfologia (`morph.rs`, `reader.rs`, `LexiconTab`):** `<WT...>` exposto em `UiNode::Strong.morph` preservando `UnknownTag`; `morph_decode(code, lang?)` lê dicionários `morph=1` do usuário (português primeiro, cache em memória, fallback para código cru). Cobertura real: `KJFiel+` 100%, `bizinterlinear.nt` 100%, `ACF 2011+` 34,8% (esquema AT fora de Robinson), `BSB` 0,6% (esquema distinto); 200 consultas em release: `p50=0,001 ms`, `p95=0,391 ms`.
