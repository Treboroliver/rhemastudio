# Apêndice de Schemas e Formatos Reais dos Módulos theWord

> **Documento gerado na tarefa M1.1:** Resultado das inspeções em módulos reais com `rhema-cli inspect`.
> **Fontes reais inspecionadas:**
> - Bíblia Texto: `kjv.ont` (7.700.722 bytes, 31.102 versículos)
> - Dicionário Strong SQLite: `Mickelson's Strong.dct.twm` (13.243.392 bytes, 14.320 verbetes)
> - Comentário AT SQLite: `Holman-AT.cmt.twm` (133.131.264 bytes, 10.858 tópicos)
> - Comentário NT SQLite: `NICNT.cmt.twm` (98.189.312 bytes, 5.785 tópicos)
> - Bíblia Encriptada DRM: `acf2007.ontx` (1.464.432 bytes, assinatura `TWENCBMOD`)

---

## 1. Módulos de Comentário (`.cmt.twm`)

### 1.1 DDL Real do SQLite

```sql
-- Tabela de metadados de configuração do módulo
CREATE TABLE config (
    name TEXT,
    value TEXT
);

-- Tabela que vincula cada tópico/comentário à referência bíblica
CREATE TABLE bible_refs (
    topic_id INTEGER PRIMARY KEY AUTOINCREMENT,
    bi INTEGER,             -- Book Index: 1 a 66 (Cânon protestante padrão)
    ci INTEGER,             -- Chapter Index: 0 para intro do livro, 1..N para capítulo
    fvi INTEGER,            -- From Verse Index: 0 para intro do capítulo, 1..N para versículo
    tvi INTEGER,            -- To Verse Index: 0 para intro, ou versículo final da faixa
    content_type TEXT       -- Normalmente 'rtf'
);

-- Índices de consulta rápida
CREATE INDEX idx_bible_refs ON bible_refs (bi, ci, fvi, tvi);
CREATE INDEX idx_bible_refs_bi_ci ON bible_refs (bi, ci);

-- Tabela com o texto formatado do comentário
CREATE TABLE content (
    topic_id INTEGER PRIMARY KEY,
    data BLOB,              -- Declaração DDL; storage class observada pode ser TEXT
    data2 BLOB              -- NULL ou dados secundários
);

-- Tabela opcional presente em alguns módulos para busca em texto puro (UTF-16LE sem RTF)
CREATE TABLE content_search (
    topic_id INTEGER PRIMARY KEY,
    data BLOB               -- Texto puro em UTF-16LE sem comandos RTF
);
```

### 1.2 Mapeamento de Referências Bíblicas em `bible_refs`

| Tipo de Entrada | `bi` | `ci` | `fvi` | `tvi` | Exemplo Real | Significado |
|---|---|---|---|---|---|---|
| **Introdução de Livro** | `1..66` | `0` | `0` | `0` | `bi=1, ci=0, fvi=0, tvi=0` | Introdução Geral de Gênesis |
| **Introdução de Capítulo** | `1..66` | `1..N` | `0` | `0` | `bi=1, ci=7, fvi=0, tvi=0` | Introdução a Gênesis 7 |
| **Versículo Único** | `1..66` | `1..N` | `v` | `v` (`fvi == tvi`) | `bi=1, ci=1, fvi=1, tvi=1` | Comentário específico de Gênesis 1:1 |
| **Faixa / Perícope** | `1..66` | `1..N` | `v1` | `v2` (`fvi < tvi`) | `bi=40, ci=1, fvi=1, tvi=17` | Mateus 1:1-17 (Genealogia) |

**Algoritmo de Consulta de Comentários para um Versículo `(b, c, v)`:**
```sql
SELECT c.topic_id, b.bi, b.ci, b.fvi, b.tvi, c.data, b.content_type
FROM bible_refs b
JOIN content c ON c.topic_id = b.topic_id
WHERE b.bi = :b
  AND (
      (b.ci = 0 AND b.fvi = 0 AND b.tvi = 0)                     -- Intro do livro
   OR (b.ci = :c AND b.fvi = 0 AND b.tvi = 0)                     -- Intro do capítulo
   OR (b.ci = :c AND b.fvi <= :v AND (b.tvi >= :v OR b.tvi = 0)) -- Versículo ou faixa
  )
ORDER BY
    -- Da maior especificidade para a mais ampla:
    -- 1. Versículo exato
    -- 2. Faixa curta
    -- 3. Intro do capítulo
    -- 4. Intro do livro
    CASE 
        WHEN b.fvi = :v AND b.tvi = :v THEN 1
        WHEN b.fvi > 0 THEN 2
        WHEN b.ci > 0 THEN 3
        ELSE 4
    END,
    (b.tvi - b.fvi) ASC;
```

---

## 2. Módulos de Dicionário / Léxico (`.dct.twm`)

### 2.1 DDL Real do SQLite

```sql
-- Metadados de configuração
CREATE TABLE config (
    name TEXT,
    value TEXT
);

-- Tabela de tópicos / verbetes
CREATE TABLE topics (
    id INTEGER PRIMARY KEY,
    pid INTEGER,                    -- Parent ID (0 para raiz, >0 para subtópicos)
    subject TEXT,                   -- Chave do verbete (ex.: "G1", "H3068", "Justificação")
    rel_order INTEGER DEFAULT 1,    -- Ordem de exibição relativa
    content_type STRING,            -- 'rtf', 'rvf' ou NULL
    strong_orig_word TEXT           -- Palavra lema original em grego/hebraico (Unicode)
);

CREATE INDEX idx_topics_subject ON topics (subject);
CREATE INDEX idx_topics_rel_order ON topics (rel_order);

-- Conteúdo do verbete
CREATE TABLE content (
    topic_id INTEGER PRIMARY KEY,
    data TEXT,                      -- Declaração DDL; Mickelson observado como BLOB
    data2 BLOB
);

-- Sinalizadores e índices de busca
CREATE TABLE topics_flags (
    id INTEGER PRIMARY KEY,
    hidden INT
);

CREATE TABLE topics_wordindex (
    id INTEGER,
    word TEXT,
    priority INTEGER
);
CREATE INDEX idx_topics_wordindex_word ON topics_wordindex (word, priority);
```

### 2.2 Formato das Chaves de Strong

- **Strong Grego:** Chave `subject = "G{n}"` sem zeros à esquerda (ex.: `G1`, `G25`, `G846`, `G2532`).
- **Strong Hebraico:** Chave `subject = "H{n}"` sem zeros à esquerda (ex.: `H1`, `H430`, `H3068`).
- **Lema Original:** Campo `strong_orig_word` armazena a palavra em Unicode original (ex.: `G1` -> `α`, `G2` -> `Ἀαρών`, `H1` -> `אָב`, `H430` -> `אֱלֹהִים`).

**Consulta de Verbete Strong:**
```sql
SELECT t.id, t.subject, t.strong_orig_word, c.data, t.content_type
FROM topics t
JOIN content c ON c.topic_id = t.id
WHERE t.subject = :key   -- ex: 'G25' ou 'H430'
LIMIT 1;
```

### 2.3 Conteúdo e RTF observados na M1.3b

A amostra foi distribuída entre o início, o meio e o fim da tabela (`N=2.000` por módulo), sem registrar texto dos módulos. O comando `rhema-cli rtf-sample` grava somente valores textuais em pasta temporária e imprime no máximo três prefixos hexadecimais para BLOBs opacos.

| Módulo | `content.data` em runtime | `content.data2` | Campo `content_type` | `config.content.type` |
|---|---|---|---|---|
| Mickelson's Strong | 14.300 BLOBs; 5.278.769 bytes no total; amostra de 2.000 sem cabeçalho RTF reconhecido | NULL em 14.300/14.300 | `topics.content_type`: NULL em 14.299, `rvf` em 1 | Ausente |
| Holman-AT | 10.857 TEXT e 1 NULL (apesar da declaração `BLOB`) | NULL em 10.858/10.858 | `bible_refs.content_type`: `rtf` em 10.857, `rvf` em 1 | `rtf` |
| NICNT | 5.785 TEXT (apesar da declaração `BLOB`) | NULL em 5.785/5.785 | `bible_refs.content_type`: `rtf` em 5.785 | Ausente |

`content_type` é o formato indicado por tópico/referência (`rtf` ou `rvf` nos valores observados); `config.content.type`, quando existe, declara o formato padrão do módulo. A entrada `rvf` de Holman mostra por que a coluna por linha deve ser considerada. `data2` permaneceu NULL em todas as linhas consultadas; sua finalidade não foi confirmada. O significado de NULL como formato padrão também não foi confirmado.

Os comentários são RTF textual sem compressão observada. Nos 2.000 registros de cada comentário, `\ansicpg0` apareceu em todas as entradas. `\'hh` foi decodificado como Windows-1252; `\uN` representa unidade Unicode UTF-16 com sinal e `\ucN` define quantos caracteres de fallback descartar. NICNT contém caracteres gregos e hebraicos em Unicode; a contagem convertida na amostra release foi 117.089 gregos e 1.186 hebraicos. Holman apresentou dois caracteres hebraicos e nenhum grego na mesma amostra. `\fcharset` apareceu 216 vezes em Holman (215 com valor 0 e uma com 128) e não apareceu no NICNT; não se atribuiu significado de idioma a esses valores.

Campos de hiperlink aparecem como grupos `\field` com destinos `\fldinst` e resultado visível `\fldrslt`. O fragmento de destino bíblico observado foi `tw://bible.*?id=...|_AUTODETECT_|`. Na amostra release, o conversor identificou 2.164 links bíblicos e reteve 25 destinos `tw://` não classificados em Holman; no NICNT identificou 59.658 links bíblicos e reteve 1.239 destinos `tw://` não classificados. Nenhum link de dicionário foi identificado nesses comentários. Não foi confirmada uma marcação real de link para entrada de dicionário.

Os BLOBs Mickelson não foram tratados como RTF: as 2.000 entradas da amostra foram classificadas como binárias pelo conversor e retornaram árvore vazia. A investigação posterior M1.3c encontrou configuração explícita de proteção; portanto, nenhuma rotina de decodificação pode ser aplicada a esses dados. Consulte a seção seguinte.

### 2.4 Investigação de proteção do dicionário Mickelson (M1.3c)

A inspeção da tabela `config` do Mickelson registrou estas propriedades relevantes:

| Chave | Valor observado | Interpretação limitada à evidência |
|---|---|---|
| `schema.version` | `1` | Versão de schema |
| `type` | `1` | Tipo declarado pelo módulo |
| `lang` | `eng` | Idioma declarado |
| `compressed` | `1` | O módulo declara conteúdo comprimido; algoritmo não identificado |
| `compressed.search.data` | `1` | O módulo declara `content_search.data` comprimido |
| `do.encrypt` | `1` | Indicação explícita de encriptação |
| `secure` | `1` | Indicação explícita de proteção |
| `strong`, `strong.g`, `strong.h` | `1` | Indicadores de conteúdo/índice Strong |
| `lemma.g`, `lemma.h` | `1` | Indicadores de lema |

Também há chaves de versão de índices e metadados editoriais. Valores de campos editoriais longos (incluindo `about` e descrição) não são reproduzidos. A comparação com Holman-AT confirmou `schema.version=1`, `type=2` e `config.content.type=rtf`; não foram observadas ali as flags `do.encrypt` ou `secure`.

Como a configuração declara simultaneamente encriptação e proteção, a investigação dos payloads foi encerrada sem descompressão, decodificação ou tentativa de contorno. Os seguintes dados estruturais já obtidos foram calculados sobre bytes/tipos e não envolvem interpretação do conteúdo:

| Medida Mickelson | Resultado |
|---|---:|
| `content.data` | 14.300 valores BLOB; mínimo 252, mediana 361, máximo 4.151 bytes |
| Entropia média de `content.data` | 7,414654 bits/byte |
| `content.data2` | NULL em 14.300/14.300; finalidade desconhecida |
| `content_search.data` | 14.300 BLOBs; mínimo 132, mediana 257, máximo 2.175 bytes |
| Entropia média de `content_search.data` | 7,189163 bits/byte |
| `topics_wordindex` | 57.541 linhas; coluna `word` armazenada como texto legível, sem os valores reproduzidos |
| `topics.content_type` | NULL em 14.299 linhas; `rvf` em 1; significado do NULL desconhecido |

Os oito prefixos de oito bytes mais frequentes de `content.data` foram `C5 7D F8 CB 5B D7 75 8E` (14), `69 40 DF C9 19 31 62 B4` (9), `10 6E 81 11 1C C5 33 37` (9), `4F 0B D9 D6 4A A9 17 B9` (9), `C6 11 6D BB CC FD D5 B6` (8), `C2 4B 3B 47 58 50 6D AE` (8), `F6 89 A5 09 2F 05 6A 9E` (8) e `78 AC 49 79 39 AF 81 80` (8).

Não houve correspondência nos bytes verificados para assinaturas comuns de zlib, gzip, bzip2, xz/LZMA, zstd, BOM UTF-16 ou `TWENCBMOD`, nem classificação como ASCII/UTF-8 válido ou RTF. A ausência de assinatura não contradiz `compressed=1` e não identifica o codec. Não é possível concluir a codificação, algoritmo, significado de `data2`, semântica de `content_type` NULL ou a estrutura dos links `tw://` para verbetes. Esses pontos não serão investigados por decodificação deste módulo protegido.

O leitor preventivo `ensure_twm_unprotected` abre SQLite com `SQLITE_OPEN_READ_ONLY` e retorna `ModuleError::EncryptedModule` quando `config` contém `do.encrypt=1` ou `secure=1`. A verificação não consulta `content`. Não se implementou decodificador para esse dicionário. Nenhum texto de módulo foi copiado para esta documentação.

### 2.5 Validação complementar M1.3c

Todos os módulos listados foram verificados exclusivamente pela tabela `config` antes de qualquer consulta a `content`. Flags ausentes significam apenas que a respectiva chave não foi declarada, não uma garantia sobre licenças ou formatos.

| Módulo | Tamanho (bytes) | `do.encrypt` | `secure` | Compressão declarada |
|---|---:|---:|---:|---|
| Holman-AT | 133.131.264 | ausente | ausente | `compressed` ausente |
| NICNT | 98.189.312 | ausente | ausente | `compressed=1`; `compressed.search.data=0` |
| VidaNova | 45.006.848 | ausente | ausente | `compressed` ausente |
| Strong_AMG2 | 36.855.808 | `0` | `0` | `compressed=0`; `compressed.search.data=0` |
| Mickelson's Strong | 15.924.224 | `1` | `1` | `compressed=1`; `compressed.search.data=1` |

Mickelson foi classificado como protegido e não houve consulta às tabelas de conteúdo neste levantamento.

Em amostras distribuídas até 2.000 linhas por módulo desprotegido, a classificação `RTF` exige o prefixo RTF e a contagem de controles desconhecidos vem do conversor M1.3b:

| Módulo | Linhas amostradas | Classe SQLite de `content.data` | RTF / não RTF | Controles desconhecidos | Média release por entrada |
|---|---:|---|---:|---|---:|
| Holman-AT | 2.000 | TEXT (2.000) | 2.000 / 0 | nenhum | 0,171 ms |
| NICNT | 2.000 | TEXT (2.000) | 2.000 / 0 | nenhum | 0,948 ms |
| Strong_AMG2 | 2.000 | TEXT (1.999), BLOB (1) | 1.996 / 4 | `charscalex` 1, `clcbpat` 530, `clvertalb` 44, `clvertalc` 478, `expndtw` 15, `trkeep` 2, `trrh` 196 | 0,098 ms |
| VidaNova | 110 de 110 disponíveis | TEXT (110) | 0 / 110 | nenhum; entradas classificadas como binárias pelo conversor | 0,002 ms |

VidaNova tem apenas 110 entradas, portanto não há 2.000 para converter. Embora o SQLite classifique `data` como TEXT, o conversor RTF existente rejeitou todas as entradas como conteúdo binário/não RTF e retornou árvores vazias: acentuação portuguesa e links bíblicos não foram confirmados (zero links detectados). Não se inferiu outro codec nem se decodificou esse formato desconhecido.

No Strong_AMG2, a chave `G25` existe na coluna `topics.subject`; o conteúdo foi aceito pelo conversor. A forma estrutural da árvore foi `Bold(6), Italic(22), Link(46), Paragraph(8), Text(233)`, sem registrar valores textuais.

Na amostra NICNT, quatro entradas ainda contêm barras invertidas ou chaves no texto simples. Só os escapes estruturais foram retidos: `\\` e `\{` em uma entrada; `\{` e `\}` em cada uma das outras três. Nenhum texto do comentário foi reproduzido.

#### ARA+.ont

`rhema-cli inspect` encontrou 7.221.554 bytes, 31.102 linhas de versículos e 12 linhas de propriedades. A propriedade `charset=0` determina Windows-1252. Chaves registradas: `id`, `lang`, `charset`, `description`, `short.title`, `version.major`, `version.minor`, `version.date`, `creator`, `about`. Metadados de texto longo não são transcritos.

As contagens de marcadores estruturais válidos, filtrando falsos positivos de `<` em texto, foram:

| Família | Ocorrências | Literais distintos |
|---|---:|---:|
| `<WG…>` | 114.715 | 5.361 |
| `<WH…>` | 226.246 | 8.651 |
| `<G…>` | 28.360 | 146 |
| `<H…>` | 70.887 | 134 |
| **Total** | **440.208** | **14.292** |

As formas `<TS…>`, `<CL>`, `<WT…>` e `<RX…>` não apareceram; permanecem **[VERIFICAR]**. O tokenizador percorreu os 31.102 versículos em 1,676 s release e produziu 99.267 `UnknownTag`: 28.360 ocorrências de formas `<G…>` (146 variantes), 70.887 de `<H…>` (134 variantes) e 20 ocorrências em 16 formas misturando texto e marcação. As formas `<G…>`/`<H…>` ainda não são interpretadas pelo tokenizador; não foi inferido seu significado. A investigação revelou e corrigiu a delimitação de propriedades em linhas de continuação iniciadas por `<`, que fazia o leitor incluir propriedades como versículos.

#### Contagens de palavras de controle nos comentários

Contagem total de ocorrências em 2.000 entradas distribuídas por arquivo. As colunas com zero indicam que o controle não apareceu na amostra. Símbolos RTF relevantes, incluindo `\*`, escapes de barras/chaves e `\'hh`, também foram contados.

| Controle | Holman (2.000) | NICNT (2.000) |
|---|---:|---:|
| `\'hh` | 4.213 | 12 |
| `\*` | 11.303 | 60.902 |
| `\\` | 0 | 1 |
| `\{` | 0 | 6 |
| `\}` | 0 | 6 |
| `\additive` | 3.843 | 4 |
| `\ansi` | 2.000 | 2.000 |
| `\ansicpg` | 2.000 | 2.000 |
| `\b` | 2.871 | 3.456 |
| `\bkmkend` | 1 | 0 |
| `\bkmkstart` | 1 | 0 |
| `\blue` | 4.808 | 6.794 |
| `\brdrb` | 54 | 1 |
| `\brdrbtw` | 54 | 1 |
| `\brdrcf` | 54 | 23.297 |
| `\brdrdb` | 54 | 1 |
| `\brdrs` | 0 | 23.296 |
| `\brdrw` | 54 | 23.297 |
| `\brsp` | 54 | 1 |
| `\bullet` | 0 | 63 |
| `\caps` | 2.608 | 0 |
| `\cbpat` | 0 | 199 |
| `\cell` | 0 | 3.499 |
| `\cellx` | 0 | 6.998 |
| `\cf` | 11.081 | 163.867 |
| `\chcbpat` | 2 | 0 |
| `\clbrdrb` | 0 | 5.736 |
| `\clbrdrl` | 0 | 5.818 |
| `\clbrdrr` | 0 | 6.010 |
| `\clbrdrt` | 0 | 5.732 |
| `\clftsWidth` | 0 | 6.786 |
| `\clvmgf` | 0 | 32 |
| `\clvmrg` | 0 | 212 |
| `\clwWidth` | 0 | 6.786 |
| `\colortbl` | 2.000 | 2.000 |
| `\cs` | 3.843 | 4 |
| `\deff` | 2.000 | 2.000 |
| `\deflang` | 2.000 | 2.000 |
| `\deflangfe` | 2.000 | 2.000 |
| `\dn` | 0 | 817 |
| `\emdash` | 3 | 5.434 |
| `\endash` | 0 | 5.769 |
| `\f` | 25.218 | 321.969 |
| `\fbidis` | 2.000 | 2.000 |
| `\fcharset` | 216 | 0 |
| `\fi` | 14.606 | 8.567 |
| `\field` | 7.458 | 60.898 |
| `\fldinst` | 7.458 | 60.898 |
| `\fldrslt` | 7.458 | 60.898 |
| `\fnil` | 3.691 | 4.003 |
| `\fonttbl` | 2.000 | 2.000 |
| `\fs` | 27.104 | 317.969 |
| `\green` | 4.808 | 6.794 |
| `\i` | 4.103 | 37.504 |
| `\intbl` | 0 | 4.399 |
| `\itap` | 2.055 | 5.158 |
| `\jpegblip` | 55 | 0 |
| `\keep` | 2.978 | 0 |
| `\keepn` | 4.387 | 2 |
| `\lang` | 1 | 0 |
| `\lastrow` | 0 | 200 |
| `\ldblquote` | 140 | 53.453 |
| `\li` | 14.606 | 8.567 |
| `\line` | 0 | 161 |
| `\lquote` | 19 | 2 |
| `\ltrch` | 15.649 | 313.674 |
| `\ltrdoc` | 1.992 | 1.996 |
| `\ltrpar` | 238 | 8.192 |
| `\notvatxbx` | 1.742 | 1 |
| `\par` | 4.834 | 21.185 |
| `\pard` | 2.055 | 9.557 |
| `\pich` | 59 | 0 |
| `\pichgoal` | 59 | 0 |
| `\picscalex` | 29 | 0 |
| `\picscaley` | 29 | 0 |
| `\pict` | 59 | 0 |
| `\picw` | 59 | 0 |
| `\picwgoal` | 59 | 0 |
| `\plain` | 15.665 | 317.953 |
| `\pngblip` | 4 | 0 |
| `\qc` | 31 | 859 |
| `\qj` | 2.032 | 0 |
| `\ql` | 12.543 | 7.708 |
| `\rdblquote` | 109 | 53.216 |
| `\red` | 4.808 | 6.794 |
| `\ri` | 14.606 | 8.567 |
| `\richviewcbh` | 0 | 6.786 |
| `\richviewcbw` | 0 | 6.786 |
| `\richviewtbw` | 0 | 2.012 |
| `\row` | 0 | 1.006 |
| `\rquote` | 39 | 3 |
| `\rtf` | 2.000 | 2.000 |
| `\rtlch` | 2 | 403 |
| `\s` | 14.295 | 125 |
| `\sa` | 14.606 | 8.567 |
| `\sb` | 14.606 | 8.567 |
| `\sbasedon` | 9.902 | 15 |
| `\sl` | 5.521 | 0 |
| `\slmult` | 5.521 | 0 |
| `\snext` | 1.822 | 4 |
| `\ssemihidden` | 1.742 | 1 |
| `\strike` | 2.608 | 0 |
| `\stylesheet` | 1.742 | 1 |
| `\sunhideused` | 1.742 | 1 |
| `\super` | 8 | 0 |
| `\tab` | 87 | 0 |
| `\tqc` | 0 | 3 |
| `\tqr` | 0 | 3 |
| `\trftsWidth` | 0 | 2.012 |
| `\trgaph` | 0 | 2.012 |
| `\trleft` | 0 | 2.012 |
| `\trowd` | 0 | 2.012 |
| `\trpaddb` | 0 | 2.012 |
| `\trpaddfb` | 0 | 2.012 |
| `\trpaddfl` | 0 | 2.012 |
| `\trpaddfr` | 0 | 2.012 |
| `\trpaddft` | 0 | 2.012 |
| `\trpaddl` | 0 | 2.012 |
| `\trpaddr` | 0 | 2.012 |
| `\trpaddt` | 0 | 2.012 |
| `\trspdb` | 0 | 2.012 |
| `\trspdfb` | 0 | 2.012 |
| `\trspdfl` | 0 | 2.012 |
| `\trspdfr` | 0 | 2.012 |
| `\trspdft` | 0 | 2.012 |
| `\trspdl` | 0 | 2.012 |
| `\trspdr` | 0 | 2.012 |
| `\trspdt` | 0 | 2.012 |
| `\trwWidth` | 0 | 2.012 |
| `\tx` | 0 | 6 |
| `\u` | 26.774 | 509.908 |
| `\uc` | 9.708 | 2.022 |
| `\ul` | 1.744 | 790 |
| `\ulc` | 2 | 790 |
| `\up` | 0 | 2.583 |
| `\widctlpar` | 232 | 0 |

Os destinos/grupos especiais observados incluem `fonttbl` e `colortbl` (2.000 controle-word occurrences em cada arquivo), `stylesheet` e `notvatxbx` (1.742 em Holman; 1 em NICNT), `pict`/`jpegblip`/`pngblip` (59/55/4 em Holman), e os três elementos de campo `field`/`fldinst`/`fldrslt` (7.458 de cada em Holman; 60.898 de cada em NICNT). `\*` ocorreu 11.303 vezes em Holman e 60.902 em NICNT. Nos 2.000 registros de comentário, o conversor não contou controle nem destino desconhecido.

---

## 3. Metadados do Módulo (`config`) em Módulos SQLite

Tanto comentários quanto dicionários e livros utilizam a tabela `config(name text, value text)` para armazenar seus metadados.

| Chave (`name`) | Tipo / Exemplo | Significado no RhemaStudio |
|---|---|---|
| `title` | `"Bíblia de Estudo Holman - AT"` | Título exibido na interface |
| `abbrev` | `"Holman - AT"`, `"M-Strong"` | Sigla para abas e botões rápidos |
| `description` | Texto longo ou NULL | Descrição do módulo |
| `about` | Documento RTF | Texto explicativo da obra / direitos |
| `type` | `"1"` (Dicionário), `"2"` (Comentário), `"3"` (Livro) | Tipo de módulo |
| `strong` | `"1"` ou `"0"` | Se contém referências de Strong |
| `content.type` | `"rtf"`, `"rvf"` | Formato padrão do corpo de texto |
| `schema.version` | `"1"` | Versão do schema theWord |
| `lang` | `"eng"`, `"por"` | Código ISO do idioma |
| `creator` | `"Jonathan K Mickelson"`, `"Brother"` | Autor / compilador |
| `version.date` | `"2023-7-29"`, `"2020-05-30"` | Data da edição do módulo |

---

## 4. Bíblias em Texto Plano (`.ont`, `.ot`, `.nt`)

### 4.1 Linhas de Versículos
- **Linhas canônicas:** Exatamente 31.102 linhas no `.ont` completo.
- **Ordem dos livros:** 1 a 66 (Gênesis a Apocalipse, Cânon Protestante).
- **Codificação:** Windows-1252 (`charset=0`) ou UTF-8 (com/sem BOM). A propriedade `charset` informa o charset original do Windows (0 = ANSI/Windows-1252, 65001 = UTF-8).

### 4.2 Tags Encontradas e Correspondência `RichText / Node`

| Tag Real | Significado | Mapeamento `Node` em `rhema-core` |
|---|---|---|
| `<WG####>` | Strong Grego (ex.: `<WG2532>`, `<WG846>`) | `Node::Strong { lang: StrongLang::Greek, num: 2532 }` |
| `<WH####>` | Strong Hebraico (ex.: `<WH3068>`, `<WH430>`) | `Node::Strong { lang: StrongLang::Hebrew, num: 3068 }` |
| `<XWG####>` | Variante / Strong Grego não-traduzido | `Node::Strong { lang: StrongLang::Greek, num: n }` |
| `<FR>...<Fr>` | Palavras de Jesus (Red Letter) | `Node::WordsOfJesus(Vec<Node>)` |
| `<FI>...<Fi>` | Itálico (palavras adicionadas pelos tradutores) | `Node::Italic(Vec<Node>)` |
| `<FO>...<Fo>` | Citação do Antigo Testamento | `Node::OtQuote(Vec<Node>)` |
| `<RF>...<Rf>` | Nota de rodapé (Footnote) | `Node::Note(Vec<Node>)` |
| `<CM>` | Marca / quebra de parágrafo | `Node::Paragraph` |
| `<i>...</i>` | Tag HTML de itálico (legado em certas notas) | `Node::Italic(Vec<Node>)` |
| `<TS>...<Ts>` | Título de Seção | `Node::Heading(u8, Vec<Node>)` |

### 4.3 A Propriedade `verse.rule` (Regras de Hiperlink do theWord)

No bloco de propriedades de Bíblias como a `kjv.ont`, encontra-se frequentemente a chave `verse.rule`:
```ini
verse.rule = "<wt>(.*?)(<W([HG]\d+)>)(<W([HG]\d+)>)?(<W([HG]\d+)>)?" "<wt><a href=_STRONG_$3|_STRONG2_$5|_STRONG3_$7|_NOLINK_>$1</a>$2$4$6"
```

**O que significa:**
1. **Mecanismo original do theWord:** Consiste em um par de strings (expressão regular de captura + string de substituição). No motor de visualização do theWord (baseado em controle de rich text/HTML), essa regra era aplicada para envelopar automaticamente as palavras anteriores aos números de Strong com hiperlinks internos (ex.: `_STRONG_G25`), permitindo que o clique na palavra ou no número abrisse o popup do dicionário correspondente.
2. **Abordagem do RhemaStudio:** O RhemaStudio **não executa substituições de regex em strings para produzir HTML**. Em vez disso, nosso tokenizador (M1.3) analisa as tags tipadas nativamente, transformando-as diretamente em nós da árvore `Vec<Node>` (`Node::Strong { lang, num, children }`). A propriedade `verse.rule` é parseada e preservada em `ModuleInfo.props` para fidelidade aos metadados do módulo original.

---

## 5. Módulos Encriptados / Protegidos por DRM (`.ontx`, `.twmx`)

- **Assinatura Binária (Mágica):** Primeiros 9 bytes contêm `TWENCBMOD` (`0x54 0x57 0x45 0x4E 0x43 0x42 0x4D 0x4F 0x44`).
- **Tratamento:** O RhemaStudio detecta a assinatura e exibe erro amigável ao usuário:
  *"Este módulo utiliza encriptação proprietária com DRM do theWord e não pode ser aberto pelo RhemaStudio."*
- Cumpre a **Regra 6 do AGENTS.md** (Direitos Autorais e conformidade legal).
