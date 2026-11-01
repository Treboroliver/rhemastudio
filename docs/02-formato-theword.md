# 02 — Formato dos módulos theWord

> **Status do documento:** Especificação refinada nas tarefas **M1.1**, **M1.3b**, **M1.3c**, **M1.3e** e **M1.4**, com schemas, leitores `.twm` de comentários/dicionários, RTF, flags de proteção, RVF e códigos gramaticais observados em módulos reais. O apêndice detalhado encontra-se em `docs/02-apendice-schemas.md`.

## 1. Famílias de módulos

| Extensão | Conteúdo | Formato | Status |
|---|---|---|---|
| `.ont` | Bíblia completa (AT + NT) | Texto simples + tags + propriedades no fim | Confirmado (31.102 versículos) |
| `.ot` | Somente Antigo Testamento | idem | Confirmado (23.145 versículos) |
| `.nt` | Somente Novo Testamento | idem | Confirmado (7.957 versículos) |
| `.cmt.twm` | Comentário (ligado a versículo, faixa, capítulo ou livro) | SQLite; `content_type` por entrada; RTF ou RVF | RTF confirmado em Holman/NICNT; RVF em VidaNova e uma entrada Holman |
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
- Codificação: a codificação segue a propriedade `charset` do módulo (`0` = ANSI Windows-1252, `65001` = UTF-8, etc.); **UTF-8 não pode ser presumido** (o theWord tem raízes em Windows/Delphi e usa Windows-1252 como padrão histórico). Exceção defensiva: para `charset=0` ou `1252`, usar UTF-8 quando todas as linhas de versículo forem UTF-8 válidas e contiverem pelo menos 20 sequências multibyte de 2 bytes; registrar aviso com o nome do módulo. A mesma codificação escolhida vale para as propriedades, inclusive título e descrição. Se qualquer linha de versículo for inválida, respeitar o charset declarado. Se a propriedade `charset` for desconhecida ou ausente: tentar UTF-8 e, se inválido, Windows-1252 como fallback. Confirmado no `kjv.ont` (`charset=0`).

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
| `<G####>` após Strong grego | Código gramatical associado ao Strong anterior | `Grammar{Greek, code}` | Confirmado estruturalmente na ARA+ (sem catálogo semântico) |
| `<H####>` após Strong hebraico | Código gramatical associado ao Strong anterior | `Grammar{Hebrew, code}` | Confirmado estruturalmente na ARA+ (sem catálogo semântico) |
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
- Se não houver palavra anterior para associar, o Strong é preservado como `Strong` sem filhos. Na ARA+, 28.359/28.360 valores `<G…>` excedem a maior chave `G5624` e 70.887/70.887 valores `<H…>` excedem `H8674`. `<Gdigits>`/`<Hdigits>` numéricas imediatamente após um `Strong` da mesma língua são interpretadas como `Grammar { lang, code }` dentro desse `Strong`; sem Strong anterior correspondente ficam como `UnknownTag`. A evidência sustenta códigos gramaticais, mas não fornece um catálogo semântico dos códigos.
- `plain_text` concatena texto sem inserir separadores, remove notas e metadados de marcação, mantém o conteúdo das tags desconhecidas e omite `Grammar`.
- Para tags [VERIFICAR], o tokenizador aceita `<TS>`/`<TSn>` com `<Ts>`/`<Tsn>`, `<WT…>` como código de morfologia e `<RX…>` como uma referência canônica no formato `Livro capítulo:versículo[-versículo]`. Se `<RX…>` não puder ser interpretada, é preservada como `UnknownTag`. A inspeção da ARA+ encontrou zero ocorrências dessas quatro famílias; portanto, **continuam [VERIFICAR]**. As tags gramaticais reconhecidas são metadados, não links clicáveis para verbetes.
- Entidades e símbolos especiais: preservar; sanitizar ao renderizar (nunca injetar HTML bruto do módulo).

### 2.4 Propriedades de módulo
Ler todas as linhas `chave=valor` ao final e guardar em `HashMap<String,String>`, usando para elas a mesma codificação selecionada para os versículos. Campos que a interface usa: título, abreviação, descrição, idioma, data da versão, indicação de Strong/morfologia. Chaves ausentes nunca são erro.

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
- Resolver o formato por entrada: usar `content_type` na linha e, se for NULL, usar `config.content.type`. Não presumir que todo `content.data` seja RTF. Na M1.4, `entries_for` deve continuar devolvendo as entradas legíveis quando outra entrada tiver tipo não suportado, associando `ModuleError::UnsupportedContent(tipo)` somente à entrada afetada para exibição de aviso na interface.
- Conteúdo pode usar marcação própria, como links e formatação. Em formatos sem especificação confirmada, não decodificar por tentativa; documentar a evidência e aguardar a definição do formato antes de implementar leitor.
- RVF também aparece em dados por entrada: 32 tópicos de Strong_AMG2 e uma referência de Holman declaram `rvf`, embora `config.content.type` de Strong_AMG2/Holman seja `rtf`.

### Conversão RTF confirmada na M1.3b
- Nos comentários Holman-AT e NICNT, `content.data` contém RTF textual, normalmente iniciado por `{\rtf1`; o conteúdo não estava comprimido nas amostras.
- A codificação declarada é `\ansicpg0` (Windows-1252). `\'hh` representa bytes nessa página de códigos; `\uN` contém unidades UTF-16 assinadas e `\ucN` informa quantos caracteres de fallback ignorar.
- Grupos `fonttbl`, `colortbl`, `stylesheet`, `pict` e o destino `notvatxbx` são ignorados. Campos de link aparecem como `\field`, `\fldinst` e `\fldrslt`; o destino bíblico observado usa `tw://bible.*?id=...|_AUTODETECT_|`. No NICNT há milhares de links bíblicos; não foi encontrado link de dicionário nos três módulos amostrados.
- Grego e hebraico visíveis no NICNT são representados por `\uN`/`\ucN` e decodificados como Unicode. A amostra release de 2.000 entradas continha 117.089 caracteres gregos e 1.186 hebraicos. Não inferir uma codificação de idioma a partir de `fcharset` sem evidência adicional.
- No dicionário Mickelson, a configuração declara `compressed=1`, `do.encrypt=1` e `secure=1`. A verificação encontrou BLOBs de alta entropia sem assinaturas padrão, mas isso não identifica o codec nem autoriza decodificação. O leitor deve rejeitar este módulo como protegido antes de consultar `content`; não tentar descompactar ou descriptografar. `data2`, `content_type` NULL e links `tw://` de verbetes seguem sem semântica confirmada.
- `TwmModule::open` abre SQLite com `SQLITE_OPEN_READ_ONLY`, ativa `PRAGMA query_only=ON`, chama `ensure_twm_unprotected` antes de ler `content`, lê `config` para memória e identifica comentário/dicionário por `config.type`, com extensão como apoio. Arquivo inválido/truncado retorna erro tipado.
- Nos textos RTF, `\par` e `\line` produzem nós estruturais; formatação conhecida inclui negrito, itálico, sublinhado, tachado, caixa alta e sobrescrito/subscrito. `plain_text` ignora notas/metadados e mantém o comportamento do tokenizador M1.3.
- `decode_twm_content(content_type, bytes)` despacha somente `"rtf"` para o conversor RTF. `"rvf"` e tipos desconhecidos retornam `ModuleError::UnsupportedContent(tipo)`. O conversor exige cabeçalho RTF e retorna erro tipado para outros bytes em vez de uma árvore vazia silenciosa; grupos RTF malformados com cabeçalho continuam tolerados.
- A forma de hiperlink para entradas do dicionário não foi confirmada em conteúdo RTF real; `LinkTarget::Dictionary` é aceito pelo conversor, mas permanece sem validação real.
- VidaNova declara `content.type=rvf` e todas as 110 linhas de `bible_refs.content_type` são `rvf`; o prefixo dos dados é próprio do RVF, não RTF, HTML ou texto simples. A especificação pública genérica RichView RVF foi consultada, mas o perfil exato do theWord não foi confirmado e não há conversor RVF.
- No verbete G25 de Strong_AMG2, foram observados 46 campos RTF `HYPERLINK`: 39 destinos `tw://` com `id` no formato estrutural `[número].[número].[número]` resolvem para versículos; 7 destinos `id=[número]` ficam como `Other`; nenhum aponta para outro verbete G/H.

### API M1.4 para comentários e dicionários

- `CommentarySource::open` carrega `bible_refs` em memória, ordenado, sem carregar `content`. `entries_for(VerseRef)` valida a referência e devolve `Result<Vec<CommentaryEntry>, ModuleError>`; cada registro contém `topic_id`, `bi`, `ci`, `fvi`, `tvi`, escopo (`Verse`, `Chapter` ou `Book`) e `content: Result<Vec<Node>, ModuleError>`. O conteúdo é lido e convertido apenas para as entradas correspondentes à consulta.
- A ordem é versículo único, faixas por largura crescente, introdução do capítulo e introdução do livro. O SQL de referência documenta `tvi=0` como limite aberto; o leitor considera esse limite sem fim, embora não tenha ocorrido nos três comentários reais contados. Registros com faixa invertida são ignorados e registrados como aviso, sem interromper a abertura.
- `DictionarySource::lookup` busca `topics.subject` sem diferenciar caixa; `strong_key(StrongLang, n)` produz `G{n}`/`H{n}`. `prefix_search(prefix, limit)` busca prefixo literal (caracteres `%`, `_` e `\\` escapados); `search_word(word, limit)` usa `topics_wordindex` e retorna `UnsupportedFormat` quando esse índice opcional não existe.
- O tipo efetivo é o `content_type` da entrada; somente NULL usa `config.content.type`. RVF ou outro tipo não suportado resulta em `UnsupportedContent` no campo `content` daquela entrada, sem descartar as demais. Nenhum payload RVF é decodificado nem representado como árvore vazia.
- CLI: `rhema-cli dump-cmt <arquivo> --book N --chapter N --verse N [--plain|--parsed]` e `rhema-cli dump-dct <arquivo> --key G25 [--plain|--parsed]`. A saída é limitada a 40 linhas e 1.500 caracteres, com aviso quando truncada.

### API M4a para o painel de estudo

- `CommentarySource::headers_for(VerseRef)` devolve `CommentaryHeader` (tópico, `bi/ci/fvi/tvi`, escopo, `content_type` efetivo, `supported`, tamanho bruto em bytes) na mesma ordem de `entries_for`, **sem ler nem converter o conteúdo**. O tamanho vem de `length(CAST(data AS BLOB))`.
- `CommentarySource::entry_page(topic_id, offset, max_paragraphs)` converte somente uma janela de parágrafos (`\par`) e devolve `RtfPage { nodes, has_more, next_offset }`. `convert_rtf_page` percorre o RTF mantendo o estado de formatação, descarta o texto anterior à janela sem criar nós e para ao encontrar conteúdo visível depois dela; `has_more` é verdadeiro somente nesse caso. Tópico desconhecido retorna `NotFound`; RVF retorna `UnsupportedContent` sem ler os dados. Uma entrada sem nenhum `\par` continua sendo uma única página (nenhuma entrada real encontrada se comportou assim).
- Escopos para a interface (`app/src-tauri/src/study.rs`): `verse` (`fvi == tvi`), `range` (`fvi < tvi`, `tvi = 0` vai até o fim do capítulo), `chapter` (faixa que cobre o capítulo inteiro), `chapterIntro` (`fvi = 0`) e `bookIntro` (`ci = 0`).
- `normalize_strong_key("g0025")`, `"G25a"` → `(Greek, 25)`; aceita zeros à esquerda e sufixo de letras; rejeita número zero, sem dígitos ou com mais de 5 dígitos.
- Links RTF para a interface: `Bible` → `verseLink` (início da faixa; a leitura navega), `Dictionary` com chave G/H → `entryLink` (abre o verbete), qualquer outro destino (`Other`, ou `Dictionary` que não seja chave de Strong) → `link` simples com `hint` contendo só o subtipo (`person` em `tw://person…`), nunca o endereço. O único formato de link de verbete confirmado em dado sintético é `_STRONG_G25|…`; nenhum link de verbete foi observado nos módulos reais (ver seção 2.3 do apêndice), de modo que essa classificação continua sem validação real.
- Verbetes de Strong: G25, G26 e H430 de Strong_AMG2 são RTF e abrem; G3056 é uma das entradas RVF do módulo e retorna `unsupported_format`.

## 4. Strong e léxico
- Bíblia com Strong: tags `<WG…>`/`<WH…>`.
- Clique (ou Enter, com foco) no número → consulta aos dicionários prontos na ordem de preferência (`lexicon_module_ids`; sem lista salva, dicionários em português primeiro). O primeiro que tiver a chave normalizada (`G25`) responde; se um tiver a entrada em RVF e nenhum outro a tiver legível, o aviso de formato não suportado prevalece sobre "não encontrado".
- Se não houver dicionário instalado, o painel mostra "Instale um dicionário para ver o léxico" com atalho para gerenciar pastas. Módulos protegidos nunca são lidos.
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
    Grammar { lang: StrongLang, code: u32 },
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

## 6.1 Biblioteca, classificação e metadados (M2.2)

`rhema_library::Library::scan(pastas, callback)` percorre pastas com profundidade máxima 6, sem seguir links simbólicos. Só considera `.ont`, `.ot`, `.nt`, `.ontx` e `.twm`; arquivos `.exe`, `.zip` e demais extensões são ignorados. A assinatura e a extensão precisam concordar: as Bíblias são texto, `.twm` é SQLite e `.ontx` é protegido (`TWENCBMOD`); uma assinatura incompatível resulta em estado corrompido, sem interromper a varredura. No Windows, `default_folders()` usa `C:\ProgramData\The Word` quando existe; Linux/macOS não presumem diretório de módulos.

Cada `ModuleInfo` do catálogo tem id determinístico calculado de caminho, tamanho e data de modificação; caminho, tipo, estado, título, abreviação, idioma (código curto e nome de exibição), descrição curta, versão, tamanho e indicador Strong. Bíblia: percorre de trás para frente somente as linhas contíguas de propriedades no final, retendo as chaves de metadados selecionadas e limitando seus valores a 200 caracteres; linhas de versículos não são retidas. Para `charset=0`/`1252`, a varredura também valida as linhas de versículo e conta sequências multibyte; se todas forem UTF-8 válidas e houver pelo menos 20 sequências de 2 bytes, título e propriedades são decodificados como UTF-8 e um aviso é registrado. Linhas finais não correspondentes a propriedades são ignoradas apenas quando têm formato de rodapé reconhecido (marcação, comentário ou continuação indentada), até o limite de 64 linhas. Valores de propriedades não usados, incluindo `about`, não são mantidos; o percurso é limitado a 16 MiB. O título de exibição da Bíblia usa `description` (até 80 caracteres), depois `short.title` e por fim o nome do arquivo; a abreviação usa `short.title` ou nome do arquivo. Códigos conhecidos de `lang` são normalizados (por exemplo `por`/`br` → `pt` / Português, `eng` → `en` / Inglês, `grc` → `gr` / Grego, `heb` → `he` / Hebraico e `spa` → `es` / Espanhol); códigos desconhecidos são preservados. `.twm`: abre SQLite estritamente somente para leitura, chama `ensure_twm_unprotected` antes de consultar `config`, ativa `PRAGMA query_only=ON` e consulta apenas chaves necessárias via `SELECT` filtrado e `substr(value, 1, 200)`. A chave `about` nunca é consultada; nenhuma tabela `content` é lida.

O catálogo não elimina módulos por título: duplicatas permanecem separadas por caminho/id e tamanho. O caminho completo de cada item fica disponível à interface, que o apresenta como dica de pasta.

Estados do catálogo:

| Estado | Significado |
|---|---|
| `Ready` / **Pronto** | Bíblia (`.ont/.ot/.nt`), comentário (`.cmt.twm`) ou dicionário (`.dct.twm`) reconhecido e não protegido |
| `Protected` / **Protegido** | extensão `.ontx`, assinatura protegida ou flag `do.encrypt=1`/`secure=1`; nenhum conteúdo é acessado |
| `UnsupportedFormat` / **Formato não suportado (em breve)** | livros `.gbk.twm`, devocionais `.dev.twm` e outros tipos `.twm` ainda não lidos |
| `Corrupted` / **Corrompido** | arquivo vazio/truncado, assinatura/schema inválido ou erro de leitura; a varredura continua |

O catálogo `catalog-<hash-da-pasta>.json` fica sob a pasta de dados configurada pelo chamador e grava `CATALOG_VERSION` (hoje 4); cache de versão diferente ou sem versão é descartado e reconstruído sem erro. Cache por módulo é reutilizado quando caminho, tamanho e mtime coincidem; cache inválido é reconstruído. A atualização usa arquivo temporário no mesmo diretório e rename. A CLI oferece `rhema-cli library scan <pasta...> [--json] [--cache <pasta>]`, lista nome/tipo/estado/idioma/tamanho, duração e quantidade de entradas aproveitadas do cache. O callback recebe a contagem de módulos reconhecidos até então.

## 6.2 Leitura do texto bíblico na interface (M3a)

Comandos Tauri (`app/src-tauri/src/reader.rs`), todos com erro serializável `{code, message, book?, max?}`; a interface traduz o `code`:

| Comando | Resultado |
|---|---|
| `get_chapter(module_id, book, chapter)` | `ChapterDto{module_id, book, chapter, verses:[{number, nodes}]}`; roda em `spawn_blocking`; o módulo é procurado no catálogo em memória pelo id (a interface nunca envia caminhos) |
| `resolve_reference(text, lang)` | faixas (com `whole` para capítulo inteiro), erros com `max` e sugestões do `rhema-refs` |
| `book_names(lang)` | 66 livros com número, nome, abreviação, capítulos e testamento |
| `set_last_passage(passage)` | grava a última passagem lida em `settings.json` (a interface usa debounce de 500 ms) |

Códigos de erro: `module_protected`, `module_not_found`, `module_corrupted`, `module_unsupported`, `book_not_in_module` (ex.: Bíblia só NT pedindo Gênesis), `verse_not_found`, `invalid_reference`, `io_error`, `internal`. Módulos protegidos, não prontos ou que não são Bíblia são rejeitados sem abrir o arquivo. Leitores abertos ficam em cache LRU de 6 módulos (`Arc<OntReader>`, aberto fora do lock), sem recarregar o módulo a cada capítulo.

Representação `UiNode` (JSON, `type` em camelCase): `text`, `italic`, `bold`, `underline`, `strikethrough`, `caps`, `superscript`, `subscript`, `wordsOfJesus`, `otQuote`, `heading{level}`, `strong{lang,num,children}`, `note`, `link`, `paragraph`, `lineBreak`. `Grammar`, `Morph` e `XRef` são descartados; `UnknownTag` vira só os filhos; `<` e `>` que fazem parte do texto do módulo chegam à interface **sem alteração** (a interface nunca altera o texto). A interface renderiza somente por componentes Svelte (`{@html}` proibido). Palavras com Strong têm sublinhado tracejado âmbar **sem ação**: a M4 ligará o clique ao verbete; `link` também ainda não tem ação.

`ChapterDto.incomplete` é verdadeiro quando faltam versículos no capítulo: os existentes são devolvidos e a interface mostra o aviso "Alguns versículos não existem nesta Bíblia". Só se nenhum versículo existir o erro é `verse_not_found`.

Tokenizador (M3a.1): em 19 capítulos da ARA+ (livros 40, 41, 42, 43, 44, 45 e 62; 2 ocorrências por capítulo, 4 em Mc 9) havia `<` soltos antes de uma palavra com Strong (`<palavra<WGn>`, também com mais de uma palavra ou vírgula antes), `>` solto no início do versículo e `<` final sem `>`. Correção na origem: se entre o `<` e o `>` há outro `<`, o trecho até o `<` interno é texto literal e a tag começa no último `<`. Resultado (tokenizando as duas Bíblias inteiras): ARA+ 40 sinais literais em 19 capítulos (preservados de propósito; semântica desconhecida, talvez marca de texto disputado) e 1 `UnknownTag` (tag gramatical órfã `(<G…>` em 2Co 7:8, sem Strong anterior); KJV 0 sinais e 0 `UnknownTag`. Na KJV, João 3:3 chega à interface com nós `wordsOfJesus` (cor própria); a ARA+ não tem essa marcação.

Desempenho (release, Rust, todos os 1.189 capítulos, sem erros e sem `<`/`>` após a conversão): ARA+ média 0,63 ms, máximo 8,8 ms (Gn 1, inclui abrir o arquivo); KJV média 0,48 ms, máximo 5,9 ms. Interface: capítulos com muitos versículos são montados em lotes de 20 por quadro; com 176 versículos (Salmo 119, versículos sintéticos pesados) a primeira pintura levou cerca de 15–21 ms e a montagem completa em torno de 150 ms, sem bloquear a rolagem.

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
