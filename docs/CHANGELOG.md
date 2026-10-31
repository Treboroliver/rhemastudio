# Changelog — RhemaStudio

## [M3b] — 2026-10-31

### Concluído
- Leitura paralela até quatro Bíblias: adição por botão ou Ctrl+K, remoção por coluna, ativação Alt+1..4, seleção comum e alinhamento por número; erros e ausências ficam isolados por coluna. Janela abaixo de 900 px mostra no máximo duas colunas.
- Backend `get_chapters` reaproveita o cache de seis leitores e retorna linhas alinhadas com conteúdo/erro por coluna. Teste de componente usa 176 linhas sintéticas pesadas em quatro colunas e lotes iniciais de 20 linhas; montagem integral no jsdom: 421,59 ms nesta execução.
- Release com ARA+ e KJV: 1.189 capítulos alinhados, 0 capítulos com contagens diferentes; média 0,669 ms e máximo 11,940 ms por capítulo. Sem casos divergentes para listar.
- Configurações versão 2: colunas, Strong, fonte e tema; configurações antigas carregam com padrões. Strong fica sem sublinhado permanente por padrão, realça em foco/hover e mostra o tracejado âmbar com Ctrl+Shift+S; a ação de clique permanece para M4.
- Fonte bíblica de 14–32 px (padrão 20), com atalhos; temas escuro, claro e automático por tokens CSS e teste de contraste mínimo 4,5:1.
- Checklist manual atualizado. Sem conteúdo bíblico real em novos fixtures, testes, capturas ou documentação.

### Em aberto
- Executar validação manual no app com usuários e módulos reais.

## [M3a.1] — 2026-10-25

### Concluído
- Tokenizador: `<` e `>` que não formam tag válida são preservados como texto literal; formas mistas (`<palavra<WGn>`) tokenizam corretamente. Testes sintéticos por padrão e proptest atualizado.
- Interface: removida a limpeza de `<`/`>`; o texto do módulo chega intacto.
- Capítulo parcialmente ausente: versículos existentes + aviso discreto (`ChapterDto.incomplete`).
- `BiblePane`: foco e rolagem até versículos ainda não montados (lotes), `aria-setsize`/`aria-posinset`; vitest com capítulo sintético de 176 versículos.
- Testes Rust: palavras de Jesus da KJV (João 3:3) chegam como `wordsOfJesus`.
- Novo `docs/03-verificacao-manual.md`.

### Resultados (RHEMA_TEST_MODULES, release)
- ARA+: 40 `<`/`>` literais em 19 capítulos (esperado zero não se cumpre: são literais do módulo), 1 `UnknownTag`. KJV: 0 e 0.

### Em aberto
- Significado dos 40 sinais literais da ARA+; tag gramatical órfã em 2Co 7:8; Ctrl+K com teclado real (ver checklist manual).

## [M3a] — 2026-10-24

### Concluído
- `CATALOG_VERSION` (3) em `rhema-library`: cache de versão diferente ou ausente é reconstruído.
- Backend Tauri: `get_chapter` (spawn_blocking, cache LRU de 6 leitores), `resolve_reference`, `book_names`, `set_last_passage`, com erros de código estável e conversão `Node` → `UiNode`.
- Interface: `BiblePane` (versículos em lotes, seleção por clique/setas), `NodeView` sem `{@html}`, notas de rodapé acessíveis, palavras de Jesus, Strong com sublinhado (sem ação até a M4), `CommandBar` (Ctrl+K), navegação por capítulo (Ctrl+←/→) entre livros, histórico (Alt+←/→, 100 posições) e última passagem lembrada.
- Testes: Rust (um por tipo de nó, cache LRU, NT sintético, protegido, 1.189 capítulos da ARA+ e KJV sem pânico e sem `<`/`>`) e vitest (nós, painel, barra de comandos, histórico, navegação).
- Release: ARA+ média 0,63 ms/máx. 8,8 ms; KJV média 0,48 ms/máx. 5,9 ms. Capturas com texto inventado em `app/ui/screenshots/`.
- Achado: 19 capítulos da ARA+ têm `<`/`>` soltos vindos do tokenizador; removidos na conversão para a interface.

## [M2.3–M2.4] — 2026-10-24

### Concluído
- Corrigidos metadados de Bíblia em `rhema-library`: idioma normalizado com código e nome, título a partir de `description` limitado a 80 caracteres (fallback `short.title`/arquivo), abreviação com `short.title`/arquivo e duplicatas mantidas por caminho e tamanho.
- Ligada a interface ao catálogo Rust: comandos Tauri para varredura com evento de progresso, lista em cache, seleção múltipla de pastas e configurações validadas em `settings.json` com gravação atômica.
- Primeiro uso em três passos, resumo da biblioteca e seleção da Bíblia padrão. Árvore dinâmica por tipo com avisos para estados protegidos/não suportados/corrompidos; removeram-se conteúdo bíblico e números de demonstração fixos.
- Adicionadas fontes locais Source Serif 4 e IBM Plex Sans, strings pt-BR/en, CSP restritiva, capability somente de diálogo e testes unitários/componentes.
- Confirmação somente de metadados com `RHEMA_TEST_MODULES=E:\coding\modulos-teste`: ARA+ identificada como pt/Português e KJV como en/Inglês; lista de tipos/estados consistente com os módulos esperados. `cargo tauri dev` iniciou a janela RhemaStudio.

## [M2.1] — 2026-10-24

### Concluído
- Criada `rhema-refs` sem dependência de parsing pesada: resolução pt-BR/en, nomes e abreviações canônicas, dobra manual de acentos, numerais/ordinais, intervalos, listas, livros/capítulos inteiros, sugestões para entrada parcial e erros com limites canônicos.
- Adicionado formatador curto/completo, com round-trip de referências de versículo; tabela TSV com 200 casos explícitos (120 válidos, 60 inválidos e 20 parciais), todos com resultado esperado; propriedades `proptest` cobrem entradas arbitrárias e ida-e-volta.
- `rhema-cli ref '<texto>' [--lang en]` imprime resoluções, erros e sugestões; `rhema-cli dump <arquivo> --ref '<texto>'` aceita referência sem remover `--book/--chapter/--verse`.
- Abreviações, sintaxe, exemplos e comportamento documentados em `docs/05-referencias.md`. M2.2 e M2.3/M2.4 estão concluídas.
- Benchmark release do parser típico, refeito com `std::hint::black_box`: 0,255 µs/op (10.000 iterações), abaixo do limite de 50 µs.

## [M2.2] — 2026-10-24

### Concluído
- `rhema-library` percorre pastas em até seis níveis sem seguir links, classifica candidatos por assinatura/extensão, ignora `.exe`/`.zip` e mantém erros de arquivo como estado Corrompido sem parar as demais entradas.
- Metadados baratos para Bíblia e `.twm`; nas Bíblias são lidas de trás para frente apenas as linhas contíguas de propriedades, sem reter versículos. SQLite permanece somente leitura, flags de proteção são verificadas antes de `config`, apenas chaves permitidas são selecionadas e `about`/`content` nunca são consultados.
- Cache `catalog-<hash>.json` por pasta sob diretório de dados configurável, invalidação por caminho/tamanho/mtime, reparo de cache inválido, atualização temporária+rename e callback de progresso.
- CLI `rhema-cli library scan <pasta...> [--json] [--cache <pasta>]` lista catálogo, tempo e contagem de entradas reaproveitadas do cache. Pastas Windows padrão detectam `C:\ProgramData\The Word`.
- Testes sintéticos cobrem classes, proteção, inválidos/vazios/truncados, links simbólicos quando disponíveis, pasta ausente, invalidação/reparo do cache e 50 módulos com reabertura release em 8,97 ms. `.gbk`/`.dev` seguem como FormatoNaoSuportado; M2.3/M2.4 foram concluídas depois.
- Não havia `RHEMA_TEST_MODULES` definida nesta execução; a tabela/tempos de módulos reais ficaram sem medição.

## [M1.4] — 2026-10-24

### Concluído
- `TwmModule::open`: SQLite read-only + `query_only`, valida proteção antes de conteúdo, carrega config e detecta comentário/dicionário.
- `CommentarySource` carrega referências em memória, valida `VerseRef`, ordena escopos específico→amplo e busca RTF sob demanda. Erros RVF são isolados no conteúdo da entrada.
- `DictionarySource` implementa lookup de `subject` sem diferenciar caixa, chave Strong, prefixo literal e busca limitada por `topics_wordindex`.
- Gerador produz fixtures sintéticas cmt/dct RTF, entradas RVF, e módulos protegidos com `do.encrypt=1` e `secure=1`; CLI `dump-cmt`/`dump-dct` limita saída a 40 linhas/1.500 caracteres.
- Teste release nos módulos permitidos, somente leitura: Holman 30,3 ms abrir/0,033 ms João 3:16; NICNT 10,3/31,2 ms; VidaNova 40,6/0,002 ms; Strong_AMG2 6,1/3,6 ms (abertura/consulta G25). As 500 consultas aleatórias em cada comentário completaram sem pânico (1,75 s Holman; 6,95 s NICNT; 0,14 s VidaNova). Uma execução inicial de VidaNova levou 938,9 ms; a checagem de proteção então lia a configuração completa (~10,16 MB) antes de `TwmModule` carregá-la de novo. Depois que a checagem passou a selecionar somente as flags, as aberturas release ficaram abaixo de 200 ms. Holman tem duas referências com `fvi>tvi`; são ignoradas com aviso.
- `.gbk`, `.dev` e M1.5 RVF permanecem pendentes; M2 não iniciada.

## [Adendo M1.4] — 2026-10-24

### Documentado
- Tipo de conteúdo resolvido por entrada (`content_type` da linha; fallback para `config.content.type` apenas quando NULL); formato não suportado deve ser isolado à entrada, sem interromper `entries_for`.
- Distribuições documentadas no apêndice: Holman 10.857 RTF/1 RVF; NICNT 5.785 RTF; Strong_AMG2 14.167 RTF/32 RVF; VidaNova 110 RVF. Mickelson não foi consultado por estar protegido.
- Roadmap reserva M1.5 para parser RVF dependente de especificação; `.gbk` e `.dev` permanecem pendentes. Naquele adendo, M1.4 ainda não havia começado; a conclusão está registrada acima.

## [Complemento M1.3e] — 2026-10-24

### Concluído
- ARA+: `<G…>`/`<H…>` numéricas excedem os maiores Strong de cada língua em 99,999% combinado. Adicionado `Node::Grammar`, anexado ao Strong precedente correspondente, excluído de `plain_text` e distinto de links. Tokenização integral dos 31.102 versículos deixou 24 `UnknownTag` (4 G numéricas sem associação e 20 formas mistas).
- Adicionado `decode_twm_content`: somente `rtf` despacha ao conversor; RVF e tipos desconhecidos retornam `UnsupportedContent`. Conversão RTF agora rejeita dados sem cabeçalho com erro tipado, em vez de devolver árvore vazia.
- Inventário de cinco `.twm` em `E:\coding\modulos-teste`: RVF ocorre em VidaNova, 32 tópicos do Strong_AMG2 e uma referência Holman; Mickelson permaneceu apenas na configuração por proteção.
- Documentada estrutura RVF com amostras mascaradas, distribuição de pontuação/controles, referência pública genérica TRichView e plano provisório. Parser RVF não implementado; M1.4 não iniciada.

## [Complemento M1.3d] — 2026-10-24

### Concluído
- VidaNova: configuração, schema, contagens e tamanhos estruturais verificados em modo somente leitura. `content.type` e as 110 entradas declaram `rvf`; o prefixo confirma formato próprio não RTF/HTML. Como não há especificação RVF confirmada, nada foi decodificado ou convertido; acentos e links seguem indeterminados.
- ARA+: comparadas 99.247 tags `<G…>`/`<H…>` com `<WG…>`/`<WH…>` adjacentes; todos os pares têm números diferentes e 278 tags não têm `W` imediatamente anterior. As tags permanecem desconhecidas; proposta de tratamento documentada, sem implementação.
- Strong_AMG2 G25: estrutura dos 46 destinos documentada sem valores ou rótulos; 39 referências a versículos, zero verbetes G/H e 7 destinos de outro tipo.
- Atualizados `docs/02-apendice-schemas.md` e `docs/02-formato-theword.md`; nenhuma alteração de leitor/tokenizador ou início da M1.4.

## [Complemento M1.3c] — 2026-10-24

### Concluído
- Acrescentada a exigência da trava de proteção na abertura `.twm` da biblioteca à M1.4; como a biblioteca ainda não tem essa API, o requisito ficou documentado em `AGENTS.md`. Fixtures sintéticas `.twm` confirmam `EncryptedModule` para `do.encrypt=1` e `secure=1`.
- Verificados os cinco módulos solicitados somente pela tabela `config`; Mickelson foi classificado como protegido e não teve o conteúdo consultado.
- Amostras release de Holman, NICNT, VidaNova e Strong_AMG2, com contagens de armazenamento, classificação RTF, controles desconhecidos, tempos e forma estrutural de G25 do AMG2 documentadas sem texto dos módulos.
- ARA+: corrigido no leitor o limite de propriedades com continuação iniciada por `<`; 31.102 versículos e charset 0. As famílias `<TS>`, `<CL>`, `<WT…>` e `<RX…>` não apareceram, portanto continuam [VERIFICAR]. O leitor CLI passou a filtrar falsos positivos de tags originados por sinais `<` em texto.
- ARA+ gerou 99.267 `UnknownTag`, principalmente códigos `<G…>`/`<H…>` ainda não interpretados. A análise M1.3d confirmou que os números pareados são diferentes; M1.3e documentou as estatísticas que sustentam códigos gramaticais e adicionou o nó correspondente. VidaNova tem 110 entradas RVF; a estrutura foi caracterizada sem parser.

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
