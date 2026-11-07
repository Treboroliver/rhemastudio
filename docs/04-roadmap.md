# 04 — Roadmap e critérios de aceite

Marque `[x]` ao concluir. Cada milestone só termina quando **todos** os critérios de aceite passam e o DoD do `AGENTS.md` é atendido.

## Exegese — E1a
- [x] E1a.1 Núcleo lexical, extratores e ligações persistentes com proveniência; ver [09-indice-lexical.md](09-indice-lexical.md).
- [ ] E1a restante: índice textual, busca integrada e interface.

---

## M0 — Fundação
- [x] M0.1 Workspace Cargo com os 7 crates vazios e `README`
- [x] M0.2 CI: `fmt`, `clippy -D warnings`, `test`
- [x] M0.3 `rhema-core`: `Book` (66 livros, nomes e abreviações pt-BR/en), `VerseRef`, `VerseRange`, tabela canônica de versículos
- [x] M0.4 App Tauri "olá mundo" com Svelte + Tailwind e tema escuro do protótipo
- [x] M0.5 `tools/gen-fixtures` (esqueleto)

**Aceite:** `cargo test --workspace` verde; soma da tabela canônica = 31.102 (teste); `cargo tauri dev` abre janela com a barra superior do protótipo.

---

## M1 — Formatos (o coração técnico)
- [x] M1.1 **Investigação com módulos reais:** `rhema-cli inspect` (SQLite e `.ont`); preencher `docs/02-apendice-schemas.md` e fechar todos os itens `[VERIFICAR]` de `docs/02-formato-theword.md`
- [x] M1.2 Leitor `.ont/.ot/.nt` com `memmap2` + índice de offsets + propriedades
- [x] M1.3 Tokenizador de marcação → `Vec<Node>` (tolerante, com `UnknownTag`)
- [x] M1.3b Conversor tolerante de RTF para `Vec<Node>`, com `plain_text`, hyperlinks, controles desconhecidos contados e amostras reais condicionadas a `RHEMA_TEST_MODULES`
- [x] M1.3c Investigação do Mickelson: flags `do.encrypt=1`/`secure=1` detectadas e `EncryptedModule` retornado antes de acessar o conteúdo; sem decodificação de payload protegido
- [x] M1.4 Leitores `.twm`: comentário e dicionário, conforme os schemas e formatos efetivamente observados
  - O formato é resolvido por entrada: coluna `content_type` da linha; se NULL, `config.content.type`. Holman-AT e Strong_AMG2 têm entradas RVF mesmo com configuração RTF.
  - Tipo não suportado em uma entrada (incluindo RVF antes do parser) não pode falhar a consulta inteira: `entries_for` devolve as demais entradas legíveis e associa `ModuleError::UnsupportedContent` à entrada afetada para aviso na interface.
  - Registrar contagens apenas por `content_type` em cada módulo; não inferir a proporção RVF pela configuração.
  - [x] `.cmt.twm` — comentários
  - [x] `.dct.twm` — dicionários
  - [ ] `.gbk.twm` — livros (pendente)
  - [ ] `.dev.twm` — devocionais (pendente)
  - Concluído: `TwmModule::open` em somente leitura + `query_only`, trava de proteção, config/tipo, comentários com índice em memória e conteúdo sob demanda, lookup/prefixo/índice de palavras de dicionários, isolamento de RVF, CLI truncada e fixtures sintéticas RTF/protegidas.
  - Nota da M1.3b/M1.3c: o Mickelson declara compressão e proteção (`do.encrypt=1`, `secure=1`) e deve ser rejeitado sem ler payload. RVF/BLOBs não são decodificados nesta tarefa.
- [x] M1.5.1 Correções do RVF — cabeçalhos estendidos, validação UTF-16, blobs de célula; espaço de nota na leitura
- [x] M1.5 Parser RVF — `rvf.rs`, despacho em `decode_twm_content`, marcador `degraded`, especificação no apêndice; ajuste de espaços RTF (sem mudança necessária) e limpeza do sufixo `<-` em títulos/idiomas
- [ ] M1.6 Gerador de fixtures sintéticas para cada tipo
- [ ] M1.7 Testes `proptest` do tokenizador + snapshots `insta`

**Aceite:** `rhema-cli dump <bíblia.ont> --book 43 --chapter 3 --verse 16` imprime o versículo e os Strong; `rhema-cli dump-cmt <comentario.twm> --book 43 --chapter 3 --verse 16` e `rhema-cli dump-dct <dicionario.twm> --key G25` imprimem as entradas em formato compatível; nenhum pânico em entradas aleatórias; abertura de `.ont` < 200 ms.

---

## M2 — Biblioteca e referências
- [x] M2.1 `rhema-refs`: parser pt-BR/en (`Jo 3:16-18`, `1Co 13`, `Sl 23:1-6; 27:1`, `Gn 1.1`, nomes completos, sem acento)
  - Concluído: validação pelo cânon, referências parciais, intervalos entre capítulos, listas, sugestões, formatação curta/completa, CLI `ref` e `dump --ref`; 200 casos explícitos, testes de propriedade e dobra manual de acentos. Ver `docs/05-referencias.md`.
- [x] M2.2 `rhema-library`: varredura de pastas, classificação, cache de catálogo
  - Concluído: varredura sem seguir links (máximo seis níveis), estados Pronto/Protegido/FormatoNaoSuportado/Corrompido, metadados sem ler conteúdo, cache JSON atômico invalidado por caminho/tamanho/mtime, progresso e CLI `library scan`. `.gbk` e `.dev` permanecem listados como "em breve".
- [x] M2.3 Tela de primeiro uso: achar módulos automaticamente ou escolher pasta
  - Concluído: fluxo de três passos, seleção múltipla de pastas, varredura com progresso, resumo de módulos e seleção opcional da Bíblia padrão; preferências em `settings.json` com validação e gravação atômica.
- [x] M2.4 Árvore de biblioteca na interface
  - Concluído: biblioteca ligada ao catálogo Rust, agrupada por tipo, seleção persistente de Bíblia, aviso para módulos protegidos/não suportados/corrompidos, dicas com caminho e navegação por teclado.

**Aceite:** ≥ 95% dos casos de uma tabela de 200 referências de teste resolvem corretamente; ao abrir o app pela primeira vez, módulos instalados aparecem agrupados por tipo; módulo corrompido aparece com aviso, sem travar.

---

## M3 — Leitura (MVP visual)
- [x] M3.1 `BiblePane` com montagem em lotes (virtualização só se necessário), tipografia serifada, versículo selecionável
- [x] M3.2 Renderização de `Node` (itálico, palavras de Jesus, títulos, notas)
- [x] M3.3 `CommandBar` (Ctrl+K): ir para referência, alternar módulo
- [x] M3.4 Bíblias paralelas (até 4), alinhadas por número, seleção comum e erros isolados por coluna; adicionadas por botão/barra de comandos, removidas por cabeçalho, Alt+1..4 ativa a principal e as colunas restauram das configurações. Abaixo de 900 px, no máximo duas ficam visíveis.
- [x] M3.5 Navegação por capítulo (setas, atalhos), histórico voltar/avançar
- [x] M3a.1 Correções da leitura (tokenizador `<`/`>`, capítulo parcial, lotes); checklist manual em `docs/03-verificacao-manual.md`
- [x] M3.6 Temas escuro/claro/automático, tokens CSS com contraste validado por teste, tamanho do texto de 14–32 px (padrão 20) e configurações versionadas/migradas.
- [x] M3b Strong: oculto por padrão, realce em hover/foco e interruptor persistente; clique continua sem ação até M4.
- [ ] M3.7 Teste de fluxo completo (Playwright): abrir o app, ler, abrir Strong e comentário, navegar por link e recolher o painel — **pendente**, não faz parte do M4a.

**Aceite:** do app aberto a duas Bíblias lado a lado em ≤ 5 interações; leitura paralela alinhada e responsiva validada por Vitest; capítulo abre em < 30 ms.

---

## M4 — Estudo: Strong, comentários, referências
- [x] M4a.1 Auditoria de RTF, cabeçalho/controles de leitura, destaque de Jesus e limites de abertura/paginação; testes sintéticos e Vitest atualizados.
- [x] M4.1 Strong clicável → aba Léxico do painel direito (o `StrongPopover` ficou fora; o painel abre com o clique ou Enter, mesmo com o interruptor de Strong desligado)
- [x] M4.2 Comentários seguindo o versículo (específico → amplo), por módulo ativo, com conteúdo sob demanda e paginado
- [ ] M4.3 Chips de referência cruzada com cartão de prévia no hover (M4b)
- [ ] M4.4 Aba Referências (todas as ligações do versículo) (M4b)
- [x] M4.5 Estados vazios amigáveis ("Instale um dicionário para ver o léxico")
- Pendente de M4a para M4b: parser RVF (M1.5). Verbetes e comentários em RVF mostram aviso por entrada e não derrubam os demais.

**Aceite:** clicar em "amou" em João 3:16 (módulo com Strong) abre a entrada G25; selecionar um versículo atualiza o comentário em < 100 ms; sem dicionário instalado, a mensagem orienta o usuário.

---

## M5 — Busca
- [ ] M5.1 Indexação em segundo plano com barra de progresso
- [ ] M5.2 Busca por palavra/frase (sem acentos, pt-BR) com trechos realçados
- [ ] M5.3 Busca por Strong (`G26`) e por referência no mesmo campo
- [ ] M5.4 Resultados paginados; clicar leva ao versículo

**Aceite:** busca em Bíblia indexada < 100 ms; "graca" encontra "graça"; `G26` lista todos os versículos com essa palavra; indexar não bloqueia a leitura.

---

## M6 — Sermão
- [x] M6.1 Modelo `rhema-sermon` + migrações + autosave atômico + backups rotativos
- [x] M6.2 Editor em blocos (título, texto, nota, ilustração, versículo, léxico; menu `/`)
- [x] M6.3 Inserir seleção do texto bíblico com `Ctrl+Enter` (e verbete do léxico)
- [ ] M6.4 Realces coloridos e anotações por versículo
- [x] M6.5 Lista de sermões, duplicar, arquivar, buscar (sem acentos)
- [~] M6.6 Exportar Markdown (feito), PDF e DOCX (pendentes)

**Aceite:** montar sermão com título, 3 pontos e 3 versículos em < 5 min; matar o processo durante a digitação não perde nada além dos últimos 500 ms; PDF exportado abre corretamente com acentos e hebraico/grego.

---

## M7 — Púlpito e Projeção
- [ ] M7.1 Modo Púlpito (layout, fonte grande, cronômetro, navegação por teclado/pedal)
- [ ] M7.2 Janela de projeção em segundo monitor com controle pela janela principal
- [ ] M7.3 Temas de projeção (fundo, fonte, rodapé com referência)
- [ ] M7.4 Exportar esboço em PPTX

**Aceite:** com dois monitores, `Ctrl+Shift+P` abre a projeção no segundo; navegar versículos na janela principal atualiza a projeção em < 100 ms; Modo Púlpito legível a 2 m de distância (fonte ≥ 28 px) e sem barras.

---

## M8 — Polimento e distribuição
- [ ] M8.1 Auditoria de acessibilidade (teclado, contraste, leitor de tela)
- [ ] M8.2 Orçamentos de desempenho verificados em CI
- [ ] M8.3 Instaladores Windows/macOS/Linux, ícone e nome finais (checar marca registrada)
- [ ] M8.4 Backup/restauração único (sermões + anotações)
- [ ] M8.5 Documentação do usuário (guia de 1 página + vídeo curto roteirizado)
- [ ] M8.6 Teste com 3 pastores reais; ajustar o que travar

**Aceite:** pastor sem treinamento prepara um sermão em < 10 min; todos os orçamentos de `docs/03-arquitetura.md` §6 verdes; instaladores assinados gerados pelo CI.

---

## Riscos e mitigação

| Risco | Mitigação |
|---|---|
| Formato `.twm` divergir do esperado | M1.1 antes de qualquer leitor definitivo; schema real vence |
| Módulos protegidos por direitos autorais | Só ler os do usuário; fixtures sintéticas; checar licença antes de embutir |
| Tags de marcação não documentadas | `UnknownTag` preserva o conteúdo; coleta de casos reais para novos testes |
| Webview diferente por SO | Testes Playwright em 3 SOs; CSS conservador |
| Marca "theWord" | Não usar no nome; aviso de compatibilidade e não afiliação |
