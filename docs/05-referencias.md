# Referências bíblicas

## Abreviações e nomes

O parser usa os nomes e abreviações canônicas de `rhema_core::Book` como fonte principal e dobra acentos sem dependência externa. A tabela pt-BR abaixo registra as formas curtas de entrada aceitas (variações listadas em `Book::abbreviations_pt()` também são válidas):

| Grupo | Abreviações pt-BR |
|---|---|
| Pentateuco | Gn, Êx/Ex, Lv, Nm, Dt |
| Históricos | Js, Jz, Rt, 1Sm, 2Sm, 1Rs, 2Rs, 1Cr, 2Cr, Ed, Ne, Et |
| Poéticos | Jó, Sl, Pv, Ec, Ct |
| Profetas maiores | Is, Jr, Lm, Ez, Dn |
| Profetas menores | Os, Jl, Am, Ob, Jn (Jonas), Mq, Na, Hc, Sf, Ag, Zc, Ml |
| Evangelhos e história | Mt, Mc, Lc, Jo (João), At |
| Cartas de Paulo | Rm, 1Co, 2Co, Gl, Ef, Fp, Cl, 1Ts, 2Ts, 1Tm, 2Tm, Tt, Fm |
| Cartas gerais e profecia | Hb, Tg, 1Pe, 2Pe, 1Jo, 2Jo, 3Jo, Jd, Ap |

Os nomes completos aceitam acentos opcionais e diferenças de caixa: por exemplo, `Êxodo`/`exodo`, `João`/`JOAO`, `Salmo`/`Salmos` e `Judas`. Livros numerados aceitam numeral arábico junto ou separado (`1Jo`, `1 João`, `1João`), ordinal romano (`I João`) e ordinal por extenso (`Primeira de João`). Os nomes e abreviações ingleses vêm de `Book::name_en()` e `Book::abbreviations_en()`; exemplos: `John`, `Jn`, `Gen`, `1 Cor` e `Psalm`.

`Jn` é resolvido pelo idioma: Jonas em pt-BR e John em inglês. `Jo` é João; a forma acentuada `Jó` identifica Job. Abreviações isoladas com mais de um candidato, como `Jo`, produzem sugestões em vez de escolher silenciosamente um livro. Livros de capítulo único aceitam o capítulo implícito: `Jd 3` é o versículo 3; `Ob`, `Fm`, `2Jo`, `3Jo` e `Jd` sem número representam o livro inteiro.

## Sintaxe e resultados

- Separador de capítulo/versículo: `:` ou `.`; intervalos aceitam `-`, `–` ou `—`.
- `Jo 3` e `Sl 23` representam capítulos inteiros; `Judas` representa o livro inteiro.
- `Jo 3:16-18` é um intervalo no mesmo capítulo; `Jo 3:16-4:2` cruza capítulos.
- Vírgula lista versículos no mesmo capítulo (`Jo 3:16,18`); ponto e vírgula inicia outra referência (`Sl 23:1-6; 27:1`).
- Entradas parciais retornam faixas já válidas, erros tipados e sugestões separadamente. Capítulo e versículo fora do cânon indicam o máximo permitido.

API pública: `parse(text, Language) -> ParseOutcome`, `format(&VerseRange, Language, FormatStyle) -> String` e `suggest_books(prefix, Language, limit) -> Vec<Book>`, em `rhema-refs`. O parser limita entrada a 16 KiB, é determinístico e não deve entrar em pânico com texto arbitrário. A CLI oferece `rhema-cli ref '<referência>' [--lang en]` e `rhema-cli dump <arquivo> --ref '<referência>'`; `dump` mantém também a forma numérica `--book/--chapter/--verse`.

O arquivo `crates/rhema-refs/tests/refs_cases.tsv` contém 200 casos explícitos pt-BR, incluindo referências válidas, inválidas e parciais. Testes de propriedade cobrem ausência de pânico e ida-e-volta de referências de versículo formatadas.
