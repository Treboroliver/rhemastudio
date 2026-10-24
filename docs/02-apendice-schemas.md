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

`content_type` é resolvido por tópico/referência; para M1.4, NULL usa `config.content.type` como fallback. A entrada `rvf` de Holman mostra por que a configuração não deve sobrepor um valor explícito da linha. `data2` permaneceu NULL em todas as linhas consultadas; sua finalidade não foi confirmada.

O predicado de consulta documentado considera `tvi=0` um limite aberto quando `fvi>0`; essa forma não apareceu nas contagens de Holman-AT, NICNT ou VidaNova. Foram encontradas duas referências com limites invertidos (`fvi>tvi`) em Holman-AT e nenhuma em NICNT/VidaNova. O leitor ignora cada registro invertido e emite aviso, preservando as demais referências do módulo.

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
| `topics.content_type` | NULL em 14.299 linhas; `rvf` em 1; sem tipo padrão declarado na configuração |

Os oito prefixos de oito bytes mais frequentes de `content.data` foram `C5 7D F8 CB 5B D7 75 8E` (14), `69 40 DF C9 19 31 62 B4` (9), `10 6E 81 11 1C C5 33 37` (9), `4F 0B D9 D6 4A A9 17 B9` (9), `C6 11 6D BB CC FD D5 B6` (8), `C2 4B 3B 47 58 50 6D AE` (8), `F6 89 A5 09 2F 05 6A 9E` (8) e `78 AC 49 79 39 AF 81 80` (8).

Não houve correspondência nos bytes verificados para assinaturas comuns de zlib, gzip, bzip2, xz/LZMA, zstd, BOM UTF-16 ou `TWENCBMOD`, nem classificação como ASCII/UTF-8 válido ou RTF. A ausência de assinatura não contradiz `compressed=1` e não identifica o codec. Não é possível concluir a codificação, algoritmo, significado de `data2`, conteúdo efetivo associado às linhas com `content_type` NULL ou a estrutura dos links `tw://` para verbetes. A configuração Mickelson não declara `config.content.type`; esses pontos não serão investigados por decodificação deste módulo protegido.

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

VidaNova tem apenas 110 entradas, portanto não há 2.000 para converter. Embora o SQLite classifique `data` como TEXT, o conversor RTF existente rejeitou todas as entradas como conteúdo não RTF e retornou árvores vazias: acentuação portuguesa e links bíblicos não foram confirmados (zero resultados do conversor, sem valor semântico). A M1.3d identificou RVF por `content.type`, `content_type` e prefixo estrutural, mas não há especificação confirmada nem conversor; consulte a análise estrutural abaixo.

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

As formas `<TS…>`, `<CL>`, `<WT…>` e `<RX…>` não apareceram; permanecem **[VERIFICAR]**. O tokenizador percorreu os 31.102 versículos em 1,676 s release e produziu 99.267 `UnknownTag`: 28.360 ocorrências de formas `<G…>` (146 variantes), 70.887 de `<H…>` (134 variantes) e 20 ocorrências em 16 formas misturando texto e marcação. A análise M1.3e abaixo passou a classificar `<G…>`/`<H…>` numéricas depois de Strong como `Grammar`; as demais formas, incluindo as 20 mistas, continuam desconhecidas. A investigação revelou e corrigiu a delimitação de propriedades em linhas de continuação iniciadas por `<`, que fazia o leitor incluir propriedades como versículos.

#### VidaNova.cmt.twm — conteúdo RVF (M1.3d)

A leitura começou pela tabela `config`. Não há `do.encrypt`, `secure` ou `compressed`; não foi observada indicação explícita de proteção. Nenhum payload foi descomprimido ou decodificado.

| Chave | Valor curto |
|---|---|
| `abbrev` | `Vida Nova` |
| `about` | `[texto omitido; 10.164.593 bytes]` |
| `author` | `D. A. Carson` |
| `content.type` | `rvf` |
| `creator` | `Keryx Digital` |
| `description` | `Comentário Bíblico Vida Nova` |
| `id` | `VidaNova.cmt` |
| `images.allow.autoresize` | `1` |
| `isbn` | `978-85-275-0424-9` |
| `lang` | `por` |
| `preserve.fonts` | `1` |
| `publish.date` | `2009` |
| `publisher` | `Vida Nova` |
| `schema.version` | `1` |
| `search.index.ref.ver` | `47` |
| `search.index.ver` | `15` |
| `title` | `CB_Vida Nova` |
| `title.english` | NULL |
| `type` | `2` |
| `user` | `1` |
| `version.date` | `2014-10-21` |

| Tabela | Linhas | Colunas |
|---|---:|---|
| `bible_link_search` | 28 | `topic_id INTEGER`, `data TEXT` |
| `bible_refs` | 110 | `topic_id INTEGER`, `bi INTEGER`, `ci INTEGER`, `fvi INTEGER`, `tvi INTEGER`, `content_type TEXT` |
| `config` | 21 | `name TEXT`, `value TEXT` |
| `content` | 110 | `topic_id INTEGER`, `data BLOB`, `data2 BLOB` |
| `content_search` | 110 | `topic_id INTEGER`, `data BLOB` |
| `sqlite_sequence` | 1 | `name`, `seq` |

O único `content_type` distinto em `bible_refs` é `rvf` (110 linhas); a configuração também declara `content.type=rvf`. Todos os 110 `content.data` são armazenados pelo SQLite como `TEXT`, embora seus bytes não sejam UTF-8 válido. Nenhuma entrada começa por `{\rtf` ou `<`: 0/110 RTF padrão, 0/110 com `<` e 110/110 com outro prefixo. O único formato observado tem 95 prefixos distintos nos primeiros 40 bytes; o prefixo estrutural comum, com os bytes de controle e o restante variável ocultados, é `-8 1 3\r\n-9 2 0 0 2 0 1\r\n[palavra]\r\n[controle…]`. Isso sustenta a identificação como formato próprio RVF, não como HTML ou texto simples.

Em bytes retornados pelo SQLite, `content.data` tem mínimo 1.385, mediana 80.981,5 e máximo 1.572.389 bytes (18.848.948 no total); `data2` é NULL em 110/110. O arquivo tem 45.006.848 bytes porque há também 14.614.854 bytes em `content_search.data`, 274.091 em `bible_link_search.data` e 10.164.593 no valor `config.about`. A soma desses campos é 43.902.486 bytes; o restante corresponde a páginas livres e estrutura SQLite (1.024 bytes/página, 43.952 páginas, 837 livres). O `length()` SQL para `content.data` não representa os bytes completos por causa de NULs embutidos.

O conversor RTF existente rejeita as 110 entradas como não RTF. Como não há especificação RVF confirmada neste repositório, não foi criado parser, nem tentada conversão. A contagem de entradas com acentos corretos e de links bíblicos permanece **indeterminada**; os zero links relatados pelo conversor RTF são apenas zero resultados de um parser que não aceitou a entrada, não evidência de ausência de links.

#### Strong_AMG2.dct.twm — ligações de G25 (M1.3d)

G25 contém 46 campos RTF do tipo `{\field{\*\fldinst HYPERLINK "[valor]"}{\fldrslt [rótulo]}}`. Todos os destinos usam esquema `tw://` e parâmetro `id=[valor]`; 39 valores têm a forma estrutural `[número].[número].[número]|[marcador]` e são referências a versículos reconhecidas pelo conversor. Os outros 7 têm a forma `[número]` sem sufixo, não correspondem a verbetes G/H e são mantidos como `Other`. Resultado: **0** links para outros verbetes G/H, **39** para versículos e **7** de outro tipo. Nenhum destino ou rótulo real foi reproduzido.

#### ARA+.ont — posição e relação entre `<G…>` e `<WG…>` / `<H…>` e `<WH…>` (M1.3d)

As propriedades confirmadas são `about`, `charset=0`, `creator`, `description`, `id=b114`, `lang=br`, `short.title=ARA+`, `version.date=2009-10-05`, `version.major=1` e `version.minor=0`; `untranslated.strong.format` não existe. Os valores textuais de metadados foram omitidos. As ocorrências foram comparadas por adjacência textual exata entre as tags:

| Família da tag final | Número igual ao `W` imediatamente anterior | Número diferente | Sem `W` imediatamente anterior |
|---|---:|---:|---:|
| `<G…>` após `<WG…>` | 0 | 28.241 | 119 |
| `<H…>` após `<WH…>` | 0 | 70.728 | 159 |

Em relação à palavra visível (após remover tags), 28.359 tags `<G…>` e 70.885 `<H…>` ficam depois de uma palavra; nenhuma fica antes ou divide uma palavra. Uma `<G…>` e duas `<H…>` não têm palavra vizinha. Nenhuma ocorrência analisada está dentro dos spans conhecidos `<FR>…<Fr>`, `<FI>…<Fi>`, `<RF>…<Rf>`, `<FO>…<Fo>` ou `<i>…</i>`.

Exemplos estruturais, com cada palavra substituída por `[palavra]`:

| Família | Cinco exemplos |
|---|---|
| `<G…>` | `[palavra]<WG1080><G5656>` · `[palavra]<WG1080><G5681>` · `[palavra]<WG3004><G5746>` · `[palavra]<WG2258><G5713>` · `[palavra]<G5625>` |
| `<H…>` | `[palavra]<WH1254><H8804>` · `[palavra]<WH1961><H8804>` · `[palavra]<WH7363><H8764>` · `[palavra]<WH559><H8799>` · `[palavra]<H8675>` |

Registro histórico da M1.3d: com a evidência então disponível, a proposta era manter `<G…>`/`<H…>` como `UnknownTag`, sem associá-los a `Node::Strong` nem descartá-los. Essa proposta foi superada pela decisão M1.3e, após a comparação dos valores com as chaves do dicionário descrita abaixo. Seguem incertos o significado individual dos códigos, a relação semântica entre as famílias e o tratamento correto das 20 formas mistas restantes.

#### M1.3e — códigos gramaticais, despacho RTF/RVF e amostras estruturais

**Tags numéricas.** Contagens da Bíblia ARA+ por número; nenhum texto bíblico foi usado nos cálculos:

| Família | Mínimo | Máximo | Números distintos | 10 valores mais frequentes (valor: ocorrências) |
|---|---:|---:|---|---|
| `<G…>` | 1063 | 5773 | 146 | 5719: 2989; 5723: 2450; 5656: 2289; 5627: 2065; 5748: 1586; 5631: 880; 5707: 849; 5692: 807; 5660: 708; 5721: 634 |
| `<H…>` | 8675 | 8809 | 134 | 8799: 19804; 8804: 12486; 8802: 5338; 8800: 4846; 8686: 4018; 8798: 2844; 8689: 2651; 8762: 2436; 8765: 2109; 8735: 1600 |

No dicionário Strong_AMG2, o maior `topics.subject` numérico é `G5624` e `H8674`. Acima dessas chaves estão 28.359/28.360 tags G (99,996%) e 70.887/70.887 tags H (100%); combinado, 99.246/99.247 (99,999%). A hipótese de códigos gramaticais passa o limiar de 95%. Isso é evidência numérica e posicional, não uma tabela de significados validada. Tags após Strong da mesma família viram `Node::Grammar { lang, code }`, anexado ao nó `Strong` imediatamente anterior; tag sem Strong anterior da mesma língua continua `UnknownTag`. Grammar não é `Link`, e fica fora de `plain_text`.

Comparação numérica dos conjuntos `<WG…>` por referência entre ARA+ e KJV:

| Referência | ARA distintos | KJV distintos | Interseção | União | Interseção/ARA | Jaccard (interseção/união) |
|---|---:|---:|---:|---:|---:|---:|
| João 1:1 | 7 | 7 | 7 | 7 | 100% | 100% |
| João 3:16 | 21 | 20 | 20 | 21 | 95,2% | 95,2% |
| Romanos 8:28 | 12 | 13 | 12 | 13 | 100% | 92,3% |

Nenhum conteúdo de versículo foi impresso. Na execução integral da ARA+ após a mudança, restaram 24 `UnknownTag`: quatro tags G numéricas sem Strong grego imediatamente anterior e 20 formas mistas. Na execução integral da KJV restaram zero. As formas mistas não foram reinterpretadas.

**Inventário `.twm` em `E:\coding\modulos-teste`.** Consultadas somente configurações antes de qualquer tabela de conteúdo. Os flags de proteção foram verificados antes do campo de formato; Mickelson permaneceu restrito à configuração.

| Arquivo | `config.content.type` | Tipos por entrada |
|---|---|---|
| Holman-AT.cmt.twm | `rtf` | `bible_refs`: `rtf`, `rvf` |
| Mickelson's Strong.dct.twm | ausente | não consultado (protegido) |
| NICNT.cmt.twm | ausente | `bible_refs`: `rtf` |
| Strong_AMG2.dct.twm | `rtf` | `topics`: `rtf` (14.167), `rvf` (32) |
| VidaNova.cmt.twm | `rvf` | `bible_refs`: `rvf` |

Entre os quatro módulos desprotegidos, `rvf` aparece em VidaNova (110 entradas), Strong_AMG2 (32 tópicos) e Holman (uma entrada); `config.content.type` não basta para escolher o conversor. Mickelson foi verificado somente na configuração e nenhum tipo por entrada foi consultado.

**VidaNova RVF — amostras mascaradas.** Primeiros 60 bytes de cinco entradas distintas. Letras foram suprimidas como `[palavra]`, dígitos trocados por `x`, bytes de controle substituídos por `<ctrl>` e CR/LF representados como `\r`/`\n`; nenhum trecho literal de conteúdo é transcrito.

| `topic_id` | Prefixo mascarado |
|---:|---|
| 1 | `-x x x\r\n-x x x x x x x\r\n[palavra]\r\n[palavra]<ctrl><ctrl><ctrl><ctrl><ctrl><ctrl>[palavra]<ctrl><ctrl>[palavra]<ctrl>[palavra]` |
| 28 | `-x x x\r\n-x x x x x x x\r\n[palavra]\r\n&<ctrl><ctrl><ctrl><ctrl><ctrl><ctrl>[palavra]<ctrl><ctrl>[palavra]<ctrl>[palavra]` |
| 59 | `-x x x\r\n-x x x x x x x\r\n[palavra]\r\n<ctrl><ctrl><ctrl><ctrl><ctrl><ctrl><ctrl>[palavra]<ctrl><ctrl>[palavra]<ctrl>[palavra]` |
| 87 | `-x x x\r\n-x x x x x x x\r\n[palavra]\r\nx<ctrl><ctrl><ctrl><ctrl><ctrl><ctrl>[palavra]<ctrl><ctrl>[palavra]<ctrl>[palavra]` |
| 114 | `-x x x\r\n-x x x x x x x\r\n[palavra]\r\nx<ctrl><ctrl><ctrl><ctrl><ctrl><ctrl>[palavra]<ctrl><ctrl>[palavra]<ctrl>[palavra]` |

Frequências em todos os 110 valores de `content.data` (18.848.948 bytes):

| Pontuação ASCII mais frequente | Contagem | Controles | Contagem |
|---|---:|---|---:|
| `)` | 201296 | NUL | 7292130 |
| `-` | 189908 | LF | 183001 |
| `.` | 137316 | CR | 177384 |
| `,` | 78974 | OTHER control | 105980 |
| `"` | 53143 | TAB | 11874 |
| `:` | 47115 |  |  |
| `/` | 34443 |  |  |
| `(` | 29502 |  |  |
| `_` | 29427 |  |  |
| `|` | 26386 |  |  |

**Referência pública consultada.** A documentação oficial de [RichView RVF](https://www.trichview.com/help/rvf_specification.html) descreve o formato RVF genérico; não é uma especificação do perfil de módulos theWord.
- RVF é composto por registros com cabeçalho e strings/binary subsequentes; cabeçalhos incluem tipo, quantidade de strings, estilo de parágrafo, opções, modo de consulta e tag.
- `-8` é o registro de versão; `-9` representa propriedades de documento.
- A versão 1.3 é permitida pela especificação; 1.3.1+ altera detalhes de UTF-8 para tags/propriedades.
- Dados binários são possíveis; em modo de consulta 1, itens externos podem exigir callback do aplicativo.
- Leitores podem ignorar registros desconhecidos usando a quantidade de strings declarada; os comprimentos e delimitadores precisam ser respeitados.
- O cabeçalho observado `-8 1 3` é compatível com RVF 1.3 e `-9 2 0 0 2 0 1` tem forma plausível de registro de propriedades; a correspondência de cada campo no perfil theWord ainda requer confirmação.

**Proposta provisória e plano de teste (não implementado).**
1. Tratar `content_type=rvf` como fluxo binário independente, sem UTF-8/RTF heurístico; limitar toda leitura ao slice de entrada e usar cursor com checagem de overflow.
2. Validar `-8`/versão e ler registros até EOF; interpretar somente tipos documentados e variantes observadas, respeitando CRLF, contagem de strings e comprimentos binários.
3. Preservar/ignorar tipos desconhecidos conforme contagem declarada; retornar `Corrupted` para cabeçalho, contagem ou registro truncado/inconsistente; nunca converter links em alvos sem reconhecer seu esquema.
4. Fixtures sintéticas cobrem versão 1.3, campos, texto/Unicode, controles NUL/CR/LF, binários, registros desconhecidos, limites de tamanho, entradas vazias/truncadas e contagens malformadas.
5. Propriedades aleatórias validam ausência de panic/overflow e consumo limitado; testes de saída verificam `plain_text` e links sem amostras protegidas. Comparação com arquivo real autorizado deve ser somente leitura e validar estrutura/contagens, não snapshots de texto.

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

### 2.6 Contagens por tipo de conteúdo (M1.4)

Contagens de linhas agrupadas por `content_type`; nenhum payload foi lido nesta consulta. O formato efetivo usa o valor da linha e recorre a `config.content.type` somente quando NULL.

| Módulo | Tabela | `rtf` | `rvf` | NULL |
|---|---|---:|---:|---:|
| Holman-AT | `bible_refs` | 10.857 | 1 | 0 |
| NICNT | `bible_refs` | 5.785 | 0 | 0 |
| Strong_AMG2 | `topics` | 14.167 | 32 | 0 |
| VidaNova | `bible_refs` | 0 | 110 | 0 |
| Mickelson's Strong | `topics` | — | — | — |

Os quatro primeiros módulos foram verificados em modo somente leitura após checagem das flags de proteção. Mickelson está protegido e não foi consultado para esta contagem; a seção 2.3 contém estatísticas históricas, sem nova leitura nesta investigação.

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
