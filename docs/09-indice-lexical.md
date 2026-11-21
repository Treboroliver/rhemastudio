# E1a.1 — Núcleo lexical

## Normalização e consulta
- Grego: NFD, remove marcas combinantes, minúsculas e sigma final → sigma comum.
- Hebraico: NFD, remove marcas, maqqef, asterisco e direcionalidade; normaliza cinco letras finais.
- Latim: NFD, remove marcas e aplica minúsculas; identificação de escrita distingue grego, hebraico, latim, misto e ausência.
- `lex parse`: Strong `G25`, `g 25`, `H0430`; domínio/entrada `25`, `25.43`; DITAT/TWOT prefixado ou `57a`; palavras gregas, hebraicas e latinas. `25` retorna as duas interpretações numéricas.

## Extração e segurança
- Strong: chave numérica, `strong_orig_word`, primeiro parêntese e números de listas após a palavra estrutural “de”.
- Louw-Nida: inteiro = domínio; decimal = domínio e entrada; subdomínio é o identificador alfabético do cabeçalho, quando presente. Nas dez estruturas mascaradas de 300 caracteres, o cabeçalho lexical aparece após o rótulo de subdomínio e a faixa decimal entre parênteses, ou o marcador de traços; termina no primeiro trecho alfabético não grego. Ponto e vírgula separa lexemas; vírgula introduz informação gramatical, não um lexema novo. Quando não identifica a estrutura, não extrai do corpo.
- DITAT: número/letra inicial, primeira palavra hebraica da chave, transliteração e números H contíguos ao cabeçalho; extrai subentradas numeradas com seu lema e H.
- DITNT: palavras gregas da chave e G no texto completo e nos primeiros 400 caracteres, com contagens. Genérico: título normalizado, removendo tags.
- Não persiste texto de verbete; conteúdo é resolvido por linha e formato padrão. SQLite de módulo é somente leitura, com verificação de proteção antes de conteúdo; falhas e entradas indisponíveis são reportadas.

## Ligações
- Strong → DITAT: `citado no verbete`; `lemma_agrees` compara lemas hebraicos dobrados. As subentradas preservam tanto seu número quanto a chave do verbete pai.
- Strong → Louw-Nida: `lema no cabeçalho`, somente igualdade com lexema extraído; não usa o corpo.
- Strong ↔ DITNT: `lema igual`, por igualdade de lema grego; consulta reversa disponível na API.
- Arquivo SQLite informado por `--db` ou padrão na pasta local do RhemaStudio; esquema versionado, gravação transacional, IDs/caminhos, tamanho e data dos módulos. Toda reconstrução invalida os dados anteriores; consulta rejeita módulos alterados ou removidos.
- `rhema-cli lex links-build <pastas...> [--db <arquivo>]` e `rhema-cli lex links G25 [--db <arquivo>]`. O arquivo não pode substituir módulo nem banco SQLite não reconhecido.

## Medições locais — 07/11/2026

Execução release com `RHEMA_TEST_MODULES=E:\coding\modulos-teste`; 4 módulos reconhecidos, 14198 Strong, protegido ignorado. Varredura 94 ms; construção 11623 ms; SQLite 5591040 bytes. Tempos dependem da máquina.

| Medida | Resultado |
|---|---|
| Ligações | DITAT 6301; Louw-Nida 7067; DITNT 715; 19 cabeçalhos decimais não reconhecidos; 0 verbetes indisponíveis. |
| DITAT, cabeçalho principal | 2347 entradas com H ligado; 1987 têm ao menos uma igualdade (84,66%), 1883 somente iguais; 2002/2499 pares iguais (80,11%). Subentradas não entram nesse denominador. |
| DITNT | 259/309 chaves contendo grego ligam-se a pelo menos um Strong (83,82%); a extração considera cada palavra grega da chave. |
| Louw-Nida | Amostra reproduzível de 200 Strong gregos: 169 com ligação, 227 pares, média 1,135 entrada por lema; E0b usou o corpo inteiro e mediu 11,285. |

| Lema consultado (identificado por Strong) | Chaves Louw-Nida |
|---|---|
| G25 | 25.104, 25.43, 25.44 |
| G3056 | 13.115, 30.13, 33.100, 33.260, 33.51, 33.99, 56.7, 57.228, 89.18 |
| G4102 | 31.102, 31.104, 31.43, 31.85, 31.88, 33.289 |
| G5485 | 25.89, 33.350, 57.103, 88.66 |
| G4561 | 23.90, 26.7, 58.10, 8.4, 8.63, 9.11, 9.12 |

## Vinte divergências DITAT

Classificação **heurística, não validação linguística**: escrita = igualdade após omitir letras de apoio; raiz/derivada = inclusão textual com diferença ≥2; forma = uma substituição; demais = indeterminada. Letras mascaradas, `H/número DITAT: forma da chave~forma Strong`; `+` separa palavras. Não promover candidatos a equivalências sem revisão especializada.

| Categoria candidata | Cinco divergências mascaradas |
|---|---|
| Vogais ou escrita | H3676/1007: xxxxxxx~xxxx; H3734/1031: xxxx~xxxxx; H489/104: xxxxxxx~xxxxxxx; H3755/1040: xxxxxx~xxxxxxx; H3758/1043: xxxxxxxxx~xxxxxxxx |
| Raiz contra derivada | H3677/1006: xxxxxx~xxxxxx+xxxxxx; H3678/1007: xxxxxxx~xxxxxxx+xxxxxxx; H3731/1028: xxxxxxxxxx~xxxxxxxxx+xxxxxxxxx; H3730/1029: xxxxxxxxxx~xxxxxxxxx+xxxxxxxxx; H3847/1075: xxxxxx~xxxxxx+xxxxxx |
| Forma diferente | H75/10: xxxxx~xxxxx; H3664/1000: xxxxxx~xxxxxx; H3697/1013: xxxxxx~xxxxxx; H3738/1037: xxx~xxxxxx; H3859/1082: xxxxx~xxxxx |
| Outra/indeterminada | H2692/1: xxx~xxxxx+xxxxxx; H4057/1: xxx~xxxxxxx; H3728/1027: xxxxxxx~xxxxxxx; H3738/1036: xxxxxxx~xxxxxx; H3739/1036: xxxxxxx~xxxxxx |

As taxas não demonstram equivalência semântica; as ligações preservam proveniência. As 19 entradas com cabeçalho não identificado exigem análise adicional. Indexação textual e interface permanecem em E1a.

## E1a.2 — Índice textual e busca

- **Campos (tantivy):** armazenados somente `module_id`, `kind` e `key`; indexados `key_norm`, `lemma` e `lemma_norm` (multivalorados, `fold_greek`/`fold_hebrew`), `translit` (`fold_latin`), `refs` (G/H do cabeçalho) e `body` (`plain_text` com `fold_latin`, não armazenado). Louw-Nida e DITNT recebem `translit_greek` nos lemas; hebraico usa só a transliteração do módulo.
- **Consultas (`parse_query`):** Strong (`key_norm`, `refs` e `links_for_strong` com proveniência), Louw-Nida (chave exata e prefixo `domínio.N`), DITAT (número e subentrada `sub:N`), Grego/Hebraico (`lemma_norm` exato e prefixo), Latim (`translit`, `key_norm` e `body` sem acentos e sem stemming).
- **Ranking e saída:** `key > subentrada > dominio > ligacao > lemma > lemma_prefixo > translit > key_norm > refs > body` (BM25 apenas em `body`). Resultados agrupados por módulo com chaves, campos, escore e interpretação; nunca texto de verbete.
- **Invalidação e segurança:** gravação em geração temporária com troca atômica de `CURRENT`; `rhema-index.json` registra `schema_version`, raízes, id, caminho, tamanho e data; `status()` detecta módulo novo, alterado ou removido; pasta padrão fora do repositório.
- **Medições (release, 12 módulos legíveis, 1 protegido ignorado, 49.403 entradas):** varredura 430 ms; construção 29.696 ms; índice 13.717.490 bytes; abertura 2,56 ms; 200 consultas com p50 0,13 ms, p95 1,53 ms e máx 2,85 ms. `graça` e `graca` retornam o mesmo conjunto (50); `G25` (pos. 1) e `25.43` (pos. 1) aparecem no topo; `ἀγαπάω` e `agapao` casam os mesmos 3 Louw-Nida e `G25` no Strong (`agapao` soma 3 ocorrências no corpo).

## E1a.3 — HTML, desempate e total real

- **Decodificação (`rhema-twformat`):** `convert_html` trata `"html"` em `decode_twm_content_checked` (`degraded=false`), convertendo `p/br/div/li/tr/h1..h6` em `Node::Paragraph`, descartando `script/style/comentários` e tags mantendo texto, decodificando entidades, respeitando `<meta charset>`/UTF-8/Windows-1252, com limite de 1 MB e profundidade 128; conteúdo sem `content_type` (`""`/`txt`/`text`) é lido como texto simples.
- **Ranking e total:** empate de pontuação desempata por `strong > louw_nida > ditat > ditnt > generic`, depois módulo e chave natural; `total` reflete a contagem real (`Count` no tantivy) além de `shown`, e a CLI imprime `total=N (mostrando M)`.
- **Medições (release):** não lidas caíram de 5.442→0 (`Dicionário Ilustrado da Bíblia`, `html`) e 5.685→0 (`STRONGS+++`, `""`). Construção 26.225 ms; índice 19.880.850 bytes; abertura 2,47 ms; 200 consultas com p50 0,14 ms, p95 1,50 ms e máx 2,97 ms. `agapao` (total 15) e `ἀγαπάω` (total 4) têm `G25` na pos. 1; `amor` (total 1499) e `graça`/`graca` (total 1104) passam a incluir o módulo HTML (e `STRONGS+++` em `amor`).

## E1b.1 — Índice reverso palavra → Strong

- **Estrutura (`lexicon-reverse.sqlite`):** indexa Bíblias legíveis com tags `Node::Strong` gravando só contagens `(n_ws, n_w, n_s)`, sem texto bíblico; tokenização `fold_latin` (`'`/`-` internos, sem números, `<2` letras ou stopwords); troca atômica e invalidação por tamanho/data.
- **Dice e limites:** `Dice = 2*n_ws/(n_w+n_s)` somando os módulos; descarta `n_ws < 2` (salvo acerto único). Limites: tags cobrem trechos (inclusive múltiplas tags/palavra) e o índice reflete o vocabulário de `ARA+` e `kjv`.
- **Integração e medições (release):** grupo `"sugestões por palavra"` (`reverse_index`) antes dos resultados por corpo. Construção: 5.514 ms, 13.299.712 bytes, 5 protegidas ignoradas (`ARA+`: 28.503 palavras, 75.644 pares; `kjv`: 12.413 palavras, 45.217 pares). `unigênito`→`G3439` (pos. 1, Dice 0,480), `amor`→`G26` (pos. 1, Dice 0,550; `G25` pos. 10), `graça`→`G5485` (pos. 1, Dice 0,525). Busca com sugestões (200 consultas): abertura 3,31 ms, p50 0,36 ms, p95 41,86 ms.
