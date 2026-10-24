# 02 — Formato dos módulos theWord

> **Status do documento:** Especificação refinada nas tarefas **M1.1**, **M1.3b** e **M1.3c**, com schemas, RTF e flags de proteção conferidos em módulos reais. O apêndice detalhado encontra-se em `docs/02-apendice-schemas.md`.

## 1. Famílias de módulos

| Extensão | Conteúdo | Formato | Status |
|---|---|---|---|
| `.ont` | Bíblia completa (AT + NT) | Texto simples + tags + propriedades no fim | Confirmado (31.102 versículos) |
| `.ot` | Somente Antigo Testamento | idem | Confirmado (23.145 versículos) |
| `.nt` | Somente Novo Testamento | idem | Confirmado (7.957 versículos) |
| `.cmt.twm` | Comentário (ligado a versículo, faixa, capítulo ou livro) | SQLite; conteúdo RTF em texto | Confirmado em Holman e NICNT |
| `.gbk.twm` | Livro geral (capítulos/seções) | SQLite | Confirmado (SQLite) |
| `.dct.twm` | Dicionário / léxico (ex.: Strong) | SQLite; `content.data` pode ser BLOB opaco | Mickelson declara compressão e proteção; conteúdo não decodificado |
| `.dev.twm` | Devocional diário | SQLite | Confirmado (extensão) |
| `.gbk.twm` híbrido | Livro com ligações a versículos (como comentário) | SQLite | Confirmado (conceito) |
| `.ontx` / `.twmx` | Módulos Encriptados (DRM) | Binário proprietário (Magic: `TWENCBMOD`) | Não suportado (rejeição com aviso) |
| Outros `.twm` | Mapas, imagens, etc. | SQLite | Fora do MVP |

Todo `.twm` inspecionado é um **banco SQLite**, mas o tipo e a codificação de `content.data` variam por módulo. Não presuma que todo conteúdo seja RTF. Módulos com sufixo `x` (`.ontx`, `.twmx`) possuem assinatura `TWENCBMOD`; além disso, um `.twm` cuja configuração declare `do.encrypt=1` ou `secure=1` é rejeitado com `ModuleError::EncryptedModule`, antes de consultar `content`. Nunca descriptografar nem tentar contornar proteção.

Locais comuns no Windows: `C:\ProgramData\The Word\` (subpastas de Bíblias e Livros). Permitir ao usuário escolher outras pastas; em Linux/macOS, sempre perguntar a pasta.

## 2. Bíblias `.ont` / `.ot` / `.nt`

### 2.1 Estrutura de arquivo
- Texto simples, **uma linha por versículo**, na ordem canônica dos 66 livros.
- Após o último versículo, bloco de **propriedades** `chave=valor` (título, descrição, abreviação, data de versão, idioma, etc.).
- Contagem esperada de linhas de versículos (Confirmado no `kjv.ont`): `.ot` = 23.145 · `.nt` = 7.957 · `.ont` = 31.102.
- O primeiro versículo do `.nt` é Mateus 1:1; do `.ot`/`.ont`, Gênesis 1:1.
- Codificação: a codificação segue estritamente a propriedade `charset` do módulo (`0` = ANSI Windows-1252, `65001` = UTF-8, etc.); **UTF-8 não pode ser presumido** (o theWord tem raízes em Windows/Delphi e usa Windows-1252 como padrão histórico). Se a propriedade `charset` for desconhecida ou ausente: tentar UTF-8, depois Windows-1252 como fallback, e registrar um aviso no log. Confirmado no `kjv.ont` (`charset=0`).

### 2.2 Mapeamento linha → referência
Usar tabela canônica de versículos por capítulo (versificação padrão protestante, 66 livros). A tabela vive em `rhema-core` como dado estático e é validada por teste: soma = 31.102.

Se o número de linhas não bater, o módulo é marcado como **"versificação não padrão"** e abre em modo degradado (por linha) com aviso. Nunca travar.

### 2.3 Marcação dentro do versículo
Tags com status **Confirmado** foram observadas em módulos reais (incluindo `kjv.ont`); tags **[VERIFICAR]** são implementadas conforme esta especificação, mas ainda não foram confirmadas contra um módulo real.

| Tag (abre/fecha) | Significado | Nó em `RichText` | Status |
|---|---|---|
| `<WG####>` | Número de Strong grego | `Strong{Greek, n}` | Confirmado |
| `<WH####>` | Número de Strong hebraico | `Strong{Hebrew, n}` | Confirmado |
| `<WG####x>` | Variante observada no `kjv.ont` (sufixo `x` após o número) | `Strong{Greek, n}` | Confirmado |
| `<XWG####>` | Strong grego não traduzido | `Strong{Greek, n}` | Confirmado |
| `<FR>…<Fr>` | Palavras de Jesus | `WordsOfJesus` | Confirmado |
| `<FI>…<Fi>` | Itálico (palavras adicionadas) | `Italic` | Confirmado |
| `<RF>…<Rf>` | Nota de rodapé | `Note` | Confirmado |
| `<CM>` | Quebra de parágrafo | `Paragraph` | Confirmado |
| `<FO>…<Fo>` | Citação do AT | `OtQuote` | Confirmado |
| `<i>…</i>` | Itálico estilo HTML | `Italic` | Confirmado |
| `<TS>…<Ts>` (e variações de nível) | Título de seção | `Heading(nível)` | **[VERIFICAR]** |
| `<CL>` | Quebra de linha | `LineBreak` | **[VERIFICAR]** |
| `<WT…>` | Morfologia | `Morph(código)` | **[VERIFICAR]** |
| `<RX …>` | Referência cruzada | `XRef(VerseRange)` | **[VERIFICAR]** |

**Regras do tokenizador:**
- Case-sensitive (abertura/fechamento diferem pela caixa).
- **Tag desconhecida nunca falha:** vira `UnknownTag{name, raw, children}`; `raw` preserva a grafia original das tags e `children` preserva o conteúdo interno.
- Tags abertas são fechadas automaticamente no fim do versículo; fechamentos sem abertura correspondente são ignorados.
- O número de Strong se liga ao último trecho contíguo sem espaços antes da tag, preservando a pontuação adjacente. Se o texto anterior tiver várias palavras, somente a última palavra (com sua pontuação) recebe o Strong. Strong consecutivos aninham-se sobre o mesmo trecho; assim, nenhum deles desloca a associação do anterior.
- Se não houver palavra anterior para associar, o Strong é preservado como `Strong` sem filhos.
- `plain_text` concatena texto sem inserir separadores, remove notas e metadados de marcação, e mantém o conteúdo de tags desconhecidas.
- Para tags [VERIFICAR], o tokenizador aceita `<TS>`/`<TSn>` com `<Ts>`/`<Tsn>`, `<WT…>` como código de morfologia e `<RX…>` como uma referência canônica no formato `Livro capítulo:versículo[-versículo]`. Se `<RX…>` não puder ser interpretada, é preservada como `UnknownTag`. **[VERIFICAR]** o formato e os níveis reais de `<TS>`, a sintaxe de `<WT>` e a codificação de `<RX>` contra módulos reais antes de tratá-los como definitivos.
- Entidades e símbolos especiais: preservar; sanitizar ao renderizar (nunca injetar HTML bruto do módulo).

### 2.4 Propriedades de módulo
Ler todas as linhas `chave=valor` ao final e guardar em `HashMap<String,String>`. Campos que a interface usa: título, abreviação, descrição, idioma, data da versão, indicação de Strong/morfologia. Chaves ausentes nunca são erro.

## 3. Módulos `.twm` (SQLite)

**Não presuma nomes de tabelas.** A tarefa M1.1 cria o comando `rhema-cli inspect <arquivo.twm>` que:
1. abre o arquivo em somente leitura;
2. imprime `sqlite_master` (tabelas, índices, DDL);
3. mostra 5 linhas de amostra de cada tabela;
4. lista conteúdo da tabela de metadados/detalhes (título, tipo, versão).

O resultado das inspeções é colado em `docs/02-apendice-schemas.md` (um bloco por tipo: `cmt`, `gbk`, `dct`, `dev`), e **só então** se escrevem os leitores definitivos.

### Comportamento esperado por tipo
- **Comentário (`cmt`):** cada entrada vincula-se a um versículo, faixa, capítulo ou livro. Consulta: dado `VerseRef`, retornar entradas cuja faixa o contém, da mais específica para a mais ampla. Neste formato não existem parágrafos.
- **Dicionário (`dct`):** busca por chave exata (`G25`, `H1254`, ou palavra). Strong grego/hebraico são os casos principais.
- **Livro (`gbk`):** árvore de capítulos/seções; leitura sequencial; busca no texto. Híbrido: ligações a versículos aparecem como chips.
- **Devocional (`dev`):** entradas por data (dia/mês).
- Conteúdo textual pode conter marcação própria (links, formatação). Mesmo tokenizador tolerante; se o formato diferir, criar módulo `markup_twm` separado.

### Conversão RTF confirmada na M1.3b
- Nos comentários Holman-AT e NICNT, `content.data` contém RTF textual, normalmente iniciado por `{\rtf1`; o conteúdo não estava comprimido nas amostras.
- A codificação declarada é `\ansicpg0` (Windows-1252). `\'hh` representa bytes nessa página de códigos; `\uN` contém unidades UTF-16 assinadas e `\ucN` informa quantos caracteres de fallback ignorar.
- Grupos `fonttbl`, `colortbl`, `stylesheet`, `pict` e o destino `notvatxbx` são ignorados. Campos de link aparecem como `\field`, `\fldinst` e `\fldrslt`; o destino bíblico observado usa `tw://bible.*?id=...|_AUTODETECT_|`. No NICNT há milhares de links bíblicos; não foi encontrado link de dicionário nos três módulos amostrados.
- Grego e hebraico visíveis no NICNT são representados por `\uN`/`\ucN` e decodificados como Unicode. A amostra release de 2.000 entradas continha 117.089 caracteres gregos e 1.186 hebraicos. Não inferir uma codificação de idioma a partir de `fcharset` sem evidência adicional.
- No dicionário Mickelson, a configuração declara `compressed=1`, `do.encrypt=1` e `secure=1`. A verificação encontrou BLOBs de alta entropia sem assinaturas padrão, mas isso não identifica o codec nem autoriza decodificação. O leitor deve rejeitar este módulo como protegido antes de consultar `content`; não tentar descompactar ou descriptografar. `data2`, `content_type` NULL e links `tw://` de verbetes seguem sem semântica confirmada.
- Nos textos RTF, `\par` e `\line` produzem nós estruturais; formatação conhecida inclui negrito, itálico, sublinhado, tachado, caixa alta e sobrescrito/subscrito. `plain_text` ignora notas/metadados e mantém o comportamento do tokenizador M1.3.
- A forma de hiperlink para entradas do dicionário não foi confirmada em conteúdo RTF real; `LinkTarget::Dictionary` é aceito pelo conversor, mas permanece sem validação real.

## 4. Strong e léxico
- Bíblia com Strong: tags `<WG…>`/`<WH…>`.
- Clique no número → consulta ao dicionário configurado como "léxico padrão" para o idioma (`G25` → entrada).
- Se não houver dicionário instalado, o painel mostra o número e convida a instalar um módulo.
- Busca por Strong: índice invertido `strong_id → [VerseRef]` por módulo.

## 5. Modelo interno (referência)

```rust
pub enum ModuleKind { Bible, Commentary, Dictionary, Book, Devotional, Unknown }

pub struct ModuleInfo {
    pub id: String,            // hash estável do caminho + tamanho + mtime
    pub path: PathBuf,
    pub kind: ModuleKind,
    pub title: String,
    pub abbreviation: Option<String>,
    pub language: Option<String>,
    pub has_strong: bool,
    pub props: HashMap<String, String>,
}

pub enum Node {
    Text(String),
    Italic(Vec<Node>),
    Bold(Vec<Node>),
    Underline(Vec<Node>),
    Strikethrough(Vec<Node>),
    Caps(Vec<Node>),
    Superscript(Vec<Node>),
    Subscript(Vec<Node>),
    WordsOfJesus(Vec<Node>),
    OtQuote(Vec<Node>),
    Heading(u8, Vec<Node>),
    Strong { lang: StrongLang, num: u32, children: Vec<Node> },
    Morph(String),
    Note(Vec<Node>),
    XRef(VerseRange),
    Link { target: LinkTarget, children: Vec<Node> },
    Paragraph,
    LineBreak,
    UnknownTag { name: String, raw: String, children: Vec<Node> },
}

pub enum LinkTarget {
    Bible(VerseRange),
    Dictionary(String),
    Other(String),
}

pub trait BibleSource {
    fn info(&self) -> &ModuleInfo;
    fn verse(&self, r: VerseRef) -> Result<Vec<Node>, ModuleError>;
    fn chapter(&self, book: Book, ch: u16) -> Result<Vec<(u16, Vec<Node>)>, ModuleError>;
}
```

## 6. Estratégia de leitura eficiente
- `.ont`: mapear arquivo em memória (`memmap2`) e indexar **offsets de linha** na abertura (≈31 mil inteiros). Verso sob demanda; nada de carregar tudo em `String`.
- `.twm`: conexão SQLite somente leitura, `PRAGMA query_only=ON`, statements preparados em cache.
- Catálogo de módulos em cache (`catalog.json` por pasta, invalidado por mtime/tamanho) para abertura instantânea.

## 7. Fixtures e testes
- `tools/gen-fixtures` gera módulos **sintéticos**: um `.ont` mínimo com todas as tags, um `.twm` de comentário e um de dicionário (após M1.1).
- Testes de propriedade (`proptest`): qualquer string de entrada no tokenizador produz `Vec<Node>` sem pânico e preserva o texto visível.
- Teste de ouro: contagem total de versículos = 31.102 com tabela canônica.
- Módulos reais protegidos nunca entram no repositório; o desenvolvedor testa localmente com seus próprios módulos via variável `RHEMA_TEST_MODULES`.

## 8. Itens verificados na M1.1 (checklist)
- [x] Contagem de linhas de `.ot` (23.145), `.nt` (7.957), `.ont` (31.102) — Confirmado com `kjv.ont`
- [x] Lista completa de tags e suas variações de caixa — Confirmado com `kjv.ont` (485.927 tags analisadas)
- [x] Nome e formato da propriedade de charset/idioma — Confirmado (`charset=0` para Win-1252, `lang=eng`)
- [x] Schema SQLite de `cmt` e `dct` — Confirmado e documentado em `docs/02-apendice-schemas.md`
- [x] Representação de faixas de versículos nos comentários — Confirmado (`bi, ci, fvi, tvi` em `bible_refs`)
- [x] Marcação interna do conteúdo `.twm` — Confirmado (RTF `{\rtf1...}` em `content.data`)
- [x] Como Strong grego/hebraico é nomeado nas chaves do dicionário — Confirmado (`G{n}` e `H{n}` sem zeros à esquerda)
- [x] Assinatura de módulos encriptados / DRM — Confirmado (Magic `TWENCBMOD` em `.ontx`)
- [ ] Existência de módulos com Apócrifos ou versificação diferente (suporte degradado por número de linha planejado)
