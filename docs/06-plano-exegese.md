# 06 — Plano: bancada de exegese, editor de documentos e quadro

> Este plano substitui o antigo "M6b parte 2" e reorganiza o que vem depois do M6a. Onde houver conflito com `docs/04-roadmap.md`, vale este documento. Itens marcados `[VERIFICAR]` dependem do inventário da fase E0 ou de conferência de licença.

## 1. Direção

O RhemaStudio passa de "leitor + sermão" para **bancada de exegese e preparação de sermões**. A exegese do texto bíblico é o alvo principal. O app ganha três pilares novos e mantém os atuais (leitura, colunas paralelas, comentários).

| Pilar | O que é |
|---|---|
| A. Estudo de palavras | Léxicos múltiplos, com busca por número Strong, lema, transliteração, palavra do texto, domínio Louw-Nida e número DITAT/TWOT |
| B. Editor de documentos | Editor rico, familiar a quem usa o Word, para sermões e trabalhos exegéticos |
| C. Quadro (canvas) | Superfície livre para diagramar o texto: orações, setas, chaves, cartões de léxico e notas |

## 2. O que muda no que já existe

- **M6b parte 2 (sermão em blocos) está cancelada.** O editor por blocos será trocado pelo editor rico (fase E4).
- **Continua valendo do M6a:** banco `rhema.db`, salvamento transacional, cópias de segurança rotativas, exportação, comandos de listar, duplicar, arquivar e excluir.
- **Continua valendo da M6b parte 1:** lista de sermões, fila de salvamento automático, versões anteriores, botão Expandir, fluxo de exportação com diálogo nativo. Troca apenas a área de edição.
- **Reaproveitar da parte 2 original:** `get_verse_snapshot` e `get_lexicon_snapshot` (agora alimentam nós do editor).
- **Atalho:** "inserir versículo selecionado" passa de Ctrl+Enter para **Alt+Enter**, porque Ctrl+Enter é quebra de página em editores de texto.

## 3. Pilar A — estudo de palavras

### 3.1 O que o usuário pode digitar
A barra Ctrl+K e um campo de busca na aba Léxico reconhecem o tipo de entrada automaticamente:

| Entrada | Exemplo | Resolve para |
|---|---|---|
| Número Strong | `G25`, `h430` | verbete Strong |
| Lema grego ou hebraico | `ἀγαπάω`, `אָב` (com ou sem acentos e vogais) | verbetes cujo lema coincide |
| Transliteração | `agapao`, `logos` | idem |
| Palavra do texto | `amor`, `unigênito`, `love` | verbetes pela glosa e, via índice reverso, os Strong mais associados à palavra |
| Domínio Louw-Nida | `25.43`, `25` | verbetes do domínio |
| Número DITAT/TWOT | `1234a`, `DITAT 2037` | verbetes do dicionário veterotestamentário |

### 3.2 Fontes
Somente os módulos que o usuário já possui (dicionários `.dct.twm`). Nada de léxico embutido sem licença. Foram encontrados Strong AMG, Louw-Nida em português com cabeçalhos de domínio, DITAT pesquisável por número e com H em cabeçalhos, e DITNT sem G no início dos verbetes; outras instalações podem diferir.

### 3.3 Índices (tudo local)
1. **Índice de léxicos** (tantivy): campos `key`, `key_norm`, `lemma`, `lemma_norm` (sem acentos e sem vogais hebraicas), `translit`, `gloss/texto`, `module`, `kind`. Reconstruído em segundo plano, com progresso, e invalidado por tamanho e data do módulo.
2. **Índice reverso palavra → Strong**, construído a partir das Bíblias com Strong que o usuário tem (hoje ARA+ e KJV): para cada palavra normalizada, os números Strong associados e a frequência. Dá "palavra do texto → original provável".
3. **Mapas entre sistemas** (Strong ↔ TWOT, Strong ↔ Louw-Nida) **somente se saírem dos dados do usuário** (por exemplo, citações dentro dos verbetes `[VERIFICAR]`). Nenhuma tabela inventada. Quando não houver mapa, o painel mostra apenas o que o módulo permite e diz isso com clareza.

**E0/E0b/E1a.1:** NA28 disponível sem tags e nenhuma Bíblia hebraica identificada; HTML e RVF precisam de tratamento explícito. Ocorrências e comparações de lemas geram candidatos, não mapas automaticamente confiáveis. Detalhes em [inventário](07-inventario-exegetico.md), [mapeamento](08-mapeamento-lexical.md) e [núcleo lexical](09-indice-lexical.md).

### 3.4 Painel de estudo de palavra (ao clicar numa palavra)
- Um cartão por léxico (Strong/AMG, Louw-Nida, DITAT, outros), na ordem de preferência do usuário.
- **Ocorrências:** lista de versículos com o mesmo Strong, contagem por livro, clique navega a leitura.
- **Palavras do versículo (interlinear):** cada palavra do original com lema, Strong, morfologia legível e glosa. Funciona também para Bíblias sem Strong (ARC, ALM21), usando uma Bíblia com Strong da mesma versificação.
- Botão "Inserir no documento" e "Colocar no quadro".

## 4. Texto original

- Colunas de Bíblia grega e hebraica (por exemplo NA28/UBS5 e outras que o usuário tenha), com hebraico da direita para a esquerda.
- Morfologia legível ("verbo, aoristo, ativo, indicativo, 3ª pessoa, singular"), decodificada a partir do esquema do módulo `[VERIFICAR no E0]`.
- Fontes embutidas com cobertura de grego politônico e hebraico com vogais, de licença livre (por exemplo Gentium Plus e Noto Serif Hebrew) `[VERIFICAR licenças]`.
- Texto disputado: os sinais `<` e `>` da ARA+ passam a aparecer como marcador discreto com dica ("trecho ausente nos manuscritos mais antigos" só se o módulo ou a descrição confirmarem o significado).

## 5. Pilar B — editor de documentos "tipo Word"

**Meta honesta:** ser familiar para quem usa Word, não um clone 1:1. Paginação perfeita e todas as funções do Word são fora de escopo.

### 5.1 Nível 1 (obrigatório)
- Barra de ferramentas: estilos de parágrafo (Normal, Título 1 a 3, Citação), fonte e tamanho, negrito, itálico, sublinhado, tachado, sobrescrito e subscrito, cor do texto, marca-texto, alinhamento (esquerda, centro, direita, justificado), listas com marcadores e numeradas com níveis, recuo, entrelinha.
- Tabelas, imagens (arquivo local), links, quebra de página, notas de rodapé, localizar e substituir, contagem de palavras, zoom, desfazer e refazer.
- Colar do Word e de páginas web preservando a formatação básica, sem executar nada.
- Verificação ortográfica do sistema, em português do Brasil.
- Atalhos habituais (Ctrl+B, I, U, Z, Y, Ctrl+F, Ctrl+H, alinhamento) e visão de "folha" com margens.

### 5.2 Nível 2 (depois)
Paginação real em A4 e Carta, cabeçalho e rodapé, numeração de página, sumário automático, estilos editáveis, comentários. Controle de alterações **não** está previsto.

### 5.3 Nós bíblicos (o diferencial)
- **Citação de versículo:** referência, abreviação da Bíblia e texto; trocar a tradução atualiza o texto; guarda o texto salvo para funcionar sem o módulo.
- **Cartão de léxico** (Strong, Louw-Nida, DITAT).
- **Referência viva:** link que abre a leitura na passagem.
- **Nota exegética** (destaque) e **nota de rodapé**.
- **Quadro incorporado** (miniatura que abre o quadro).
- Menu `/` para inserir qualquer um deles; Alt+Enter insere o versículo selecionado.

### 5.4 Tecnologia
- ProseMirror via **TipTap** (núcleo de licença MIT; extensões "Pro" são pagas e **não** serão usadas) `[VERIFICAR licenças]`. Alternativa: Lexical. Nada de serviços em nuvem.
- Documento salvo como **JSON** com `schema_version`, mais conversores.
- **Exportar:** DOCX (biblioteca `docx` ou `docx-rs`, MIT), PDF (impressão do WebView com folha de estilos de impressão), Markdown e HTML. **Importar DOCX** (mammoth) fica para depois.
- Imagens: pasta `media/` na pasta de dados, referenciadas por hash, com limite de tamanho. Nenhuma busca de imagem na internet.

### 5.5 Layout
Modos de trabalho: **Leitura** (atual), **Documento** (editor no centro; a leitura vira painel lateral recolhível; o painel de estudo continua à direita) e **Quadro** (canvas no centro). O seletor de modo fica no cabeçalho.

## 6. Pilar C — quadro (canvas)

**Interpretação adotada:** quadro livre de diagramação exegética. Se a ideia for um estilo "Canva" (criar artes e slides), esse trecho vira parte do M7 (modelos de projeção) e este pilar encolhe.

- Peças arrastáveis: palavras e frases do texto (original ou tradução), cartões de léxico, notas, versículos inteiros.
- Setas, chaves, agrupamentos, rótulos de função (sujeito, verbo, objeto, modificador, oração subordinada) e cores.
- **Modo diagrama de orações:** importa o versículo dividido em palavras; o usuário arrasta, indenta (linha de pensamento) e liga com setas.
- Zoom e rolagem, selecionar vários, desfazer e refazer, alinhar e distribuir, atalhos de teclado.
- Salvar vinculado a uma passagem; exportar PNG, SVG e PDF; incorporar no documento.
- **Tecnologia:** decidir após um spike curto (fase E5.0) comparando Svelte Flow (nós e arestas), Konva (desenho livre) e Excalidraw (ferramenta completa, em React). Critérios: funciona offline, peso, teclado e acessibilidade, exportação, licença `[VERIFICAR]`. Recomendação provisória: Svelte Flow como base estruturada, mais uma camada simples de formas e texto.

## 7. Modelo de dados (novo)

| Tabela | Conteúdo |
|---|---|
| `documents` | id, título, tipo (`sermon`, `exegesis`, `note`), passagem, `doc_json`, `schema_version`, datas, arquivado |
| `document_versions` | cópias de segurança do `doc_json` (mesma política do M6a: no máximo 1 por minuto, 20 mais recentes) |
| `canvases` | id, título, passagem, `scene_json`, `schema_version`, miniatura, documento vinculado (opcional), datas |
| `media` | hash, tipo, tamanho, caminho relativo |

**Migração v3:** converte os blocos dos sermões atuais em `doc_json` (Título → heading, Texto → parágrafos, Versículo → nó de citação, Léxico → cartão, Nota → destaque), mantém as tabelas antigas por uma versão para poder voltar atrás e tem teste de migração.

## 8. Fases

Tamanho: P = uma sessão curta, M = uma sessão, G = pode exigir duas sessões. Cada fase termina com commit e pode ser retomada sozinha.

| Fase | Entrega | Tam. | Depende de | Aceite |
|---|---|---|---|---|
| **E0** | Inventário: léxicos, Bíblias gregas e hebraicas, formatos de chave e de morfologia (`docs/07-inventario-exegetico.md`) | P | — | Relatório responde: dá para buscar por lema, Louw-Nida, TWOT/DITAT? Qual Bíblia tem morfologia? |
| **E1a** | Índice de léxicos (tantivy) + busca por Strong, lema, transliteração, glosa e chaves dos outros dicionários | G | E0 | Digitar `agapao`, `ἀγαπάω`, `G25` e `amor` leva ao verbete; resposta em menos de 100 ms com índice pronto |
| **E1b** | Índice reverso palavra → Strong + modo "léxico" na barra Ctrl+K e campo na aba Léxico | M | E1a | "unigênito" sugere G3439; resultados agrupados por módulo |
| **E2a** | Painel de palavra: cartões por léxico e lista de ocorrências | G | E1 | Clique numa palavra mostra cartões e contagem por livro |
| **E2b** | "Palavras do versículo" (interlinear) | M | E2a, E3 se houver morfologia | Funciona também com ARC e ALM21 |
| **E3** | Colunas grega e hebraica (RTL), morfologia legível, fontes | M | E0 | Hebraico legível da direita para a esquerda; morfologia decodificada |
| **E4a** | Editor rico: núcleo, barra de ferramentas, persistência, migração v3 | G | M6a | Sermões antigos abrem convertidos; salvamento automático e versões funcionam |
| **E4b** | Nós bíblicos e menu `/`, Alt+Enter, cartão de léxico | G | E4a, E2a | Montar um sermão com 3 versículos e 1 cartão em menos de 5 minutos |
| **E4c** | Tabelas, imagens, listas com níveis, localizar e substituir, notas de rodapé, contagem de palavras | G | E4a | Colar uma página do Word mantém negrito, itálico, listas e tabelas |
| **E4d** | Exportar DOCX, PDF, Markdown e HTML; imprimir | G | E4c | DOCX abre no Word com acentos, grego e hebraico corretos |
| **E5.0** | Spike do quadro: três bibliotecas, tabela de decisão | P | — | Decisão registrada em ADR |
| **E5a** | Quadro: peças, setas, grupos, zoom, salvar | G | E5.0 | Criar, salvar, fechar, abrir; teclado funciona |
| **E5b** | Diagrama de orações, exportar imagem, incorporar no documento | G | E5a, E4b | Diagramar João 1:1 e incorporar ao documento |
| **M7** | Púlpito e projeção, usando o documento rico | G | E4 | Como em `docs/04-roadmap.md` |
| **M5, M4b, M8** | Busca de texto, referências cruzadas, instaladores e acabamento | G | — | Como em `docs/04-roadmap.md` |

### Ordem recomendada
1. **E0 → E1a → E1b → E2a → E3 → E2b** (exegese de palavra funcionando; reaproveita a maior parte do que já existe).
2. **E4a → E4b → E4d → E4c** (documento com Bíblia e exportação).
3. **E5.0 → E5a → E5b** (quadro).
4. **M7**, depois M5, M4b e M8.

Pacotes de entrega: **v0.3** = fase 1 + E4a/E4b/E4d; **v0.4** = restante do editor + quadro; **v0.5** = púlpito e projeção, busca e acabamento.

## 9. Licenças e política

- Louw-Nida e DITAT são obras protegidas. O app só lê módulos que o usuário já tem, nunca os distribui, e continua **sem tentar abrir módulos protegidos**. Se esses módulos estiverem protegidos, o app os lista com cadeado e orienta o usuário a abri-los no theWord.
- Bibliotecas e fontes: conferir a licença de cada uma antes de adicionar (TipTap e extensões, `docx`, Svelte Flow, Konva, Excalidraw, fontes de grego e hebraico). Nenhuma dependência que exija serviço em nuvem ou chave paga.
- O nome do produto (RhemaStudio) continua pendente de checagem de marca.

## 10. Riscos

| Risco | Mitigação |
|---|---|
| Louw-Nida ou DITAT ausentes, protegidos ou sem mapeamento com Strong | E0 define; sem mapa, busca por palavra e por lema nos módulos que existirem |
| "Igual ao Word" é um alvo sem fim | Nível 1 fechado; nível 2 só depois; pedidos novos entram numa lista, não no escopo |
| Documentos grandes pesam | Meta: 200 páginas sem travar; testar com documento sintético de 100 mil palavras |
| Exportar DOCX com grego e hebraico | Testes de ida e volta; fontes explícitas no arquivo |
| Escolha do canvas | Spike curto antes de investir |
| Custo de crédito de IA | Fases pequenas, commit por fase, `docs/CONTEXTO.md` como entrada única, retomada por auditoria do código |

## 11. Decisões pendentes (do autor)

1. Significado de "canva" (quadro de diagramação ou criação de artes e slides).
2. Onde estão os módulos de Louw-Nida e DITAT (se existirem) e se abrem fora do theWord.
3. Se o idioma da interface de estudo é só português ou também inglês.
