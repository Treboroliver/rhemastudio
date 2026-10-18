# 04 — Roadmap e critérios de aceite

Marque `[x]` ao concluir. Cada milestone só termina quando **todos** os critérios de aceite passam e o DoD do `AGENTS.md` é atendido.

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
- [ ] M1.1 **Investigação com módulos reais:** `rhema-cli inspect` (SQLite e `.ont`); preencher `docs/02-apendice-schemas.md` e fechar todos os itens `[VERIFICAR]` de `docs/02-formato-theword.md`
- [ ] M1.2 Leitor `.ont/.ot/.nt` com `memmap2` + índice de offsets + propriedades
- [ ] M1.3 Tokenizador de marcação → `Vec<Node>` (tolerante, com `UnknownTag`)
- [ ] M1.4 Leitores `.twm`: comentário, dicionário, livro (conforme schemas reais)
- [ ] M1.5 Gerador de fixtures sintéticas para cada tipo
- [ ] M1.6 Testes `proptest` do tokenizador + snapshots `insta`

**Aceite:** `rhema-cli dump <bíblia.ont> --ref "Jo 3:16"` imprime o versículo e os Strong; `rhema-cli dump <dicionario.twm> --key G25` imprime a entrada; nenhum pânico em 10.000 entradas aleatórias; abertura de `.ont` < 200 ms.

---

## M2 — Biblioteca e referências
- [ ] M2.1 `rhema-refs`: parser pt-BR/en (`Jo 3:16-18`, `1Co 13`, `Sl 23:1-6; 27:1`, `Gn 1.1`, nomes completos, sem acento)
- [ ] M2.2 `rhema-library`: varredura de pastas, classificação, cache de catálogo
- [ ] M2.3 Tela de primeiro uso: achar módulos automaticamente ou escolher pasta
- [ ] M2.4 Árvore de biblioteca na interface

**Aceite:** ≥ 95% dos casos de uma tabela de 200 referências de teste resolvem corretamente; ao abrir o app pela primeira vez, módulos instalados aparecem agrupados por tipo; módulo corrompido aparece com aviso, sem travar.

---

## M3 — Leitura (MVP visual)
- [ ] M3.1 `BiblePane` virtualizado, tipografia serifada, versículo selecionável
- [ ] M3.2 Renderização de `Node` (itálico, palavras de Jesus, títulos, notas)
- [ ] M3.3 `CommandBar` (Ctrl+K): ir para referência, alternar módulo
- [ ] M3.4 Bíblias paralelas (até 4)
- [ ] M3.5 Navegação por capítulo (setas, atalhos), histórico voltar/avançar
- [ ] M3.6 Temas claro/escuro e tamanho de fonte

**Aceite:** do app aberto a "João 3:16 em 2 traduções lado a lado" em ≤ 5 interações; capítulo abre em < 30 ms; Playwright cobre o fluxo.

---

## M4 — Estudo: Strong, comentários, referências
- [ ] M4.1 Strong clicável → `StrongPopover` e aba Léxico
- [ ] M4.2 Comentários seguindo o versículo (específico → amplo)
- [ ] M4.3 Chips de referência cruzada com cartão de prévia no hover
- [ ] M4.4 Aba Referências (todas as ligações do versículo)
- [ ] M4.5 Estados vazios amigáveis ("Instale um dicionário para ver o léxico")

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
- [ ] M6.1 Modelo `rhema-sermon` + migrações + autosave atômico + backups rotativos
- [ ] M6.2 Editor em blocos com menu `/` (versículo, léxico, nota, ilustração)
- [ ] M6.3 Inserir seleção do texto bíblico com `Ctrl+Enter`
- [ ] M6.4 Realces coloridos e anotações por versículo
- [ ] M6.5 Lista de sermões, duplicar, arquivar, buscar
- [ ] M6.6 Exportar Markdown, PDF e DOCX

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
