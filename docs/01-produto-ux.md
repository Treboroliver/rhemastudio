# 01 — Produto e UX

## 1. Visão

Um software de estudo bíblico **moderno, leve e simples**, que abre os módulos do theWord que o pastor já possui e o leva do texto ao púlpito sem trocar de ferramenta.

**Promessa:** "Abra o texto. Estude a palavra. Monte o sermão. Pregue."

## 2. Personas

| Persona | Contexto | O que precisa |
|---|---|---|
| **Pastor titular** | Prepara sermões no notebook, em geral na semana toda | Estudar passagem, anotar, montar esboço, imprimir/pregar |
| **Pregador itinerante** | Notebook modesto, internet ruim, prega em várias igrejas | Offline total, leve, abrir rápido, levar o sermão em PDF ou no próprio app |
| **Líder de célula / professor de EBD** | Pouca familiaridade com tecnologia | Interface óbvia, pouquíssimos botões, ajuda contextual |

## 3. Princípios de design

1. **Zero manual.** Primeiro uso guiado em ≤ 3 telas; tudo descobrível pela barra de comandos.
2. **Barra de comandos (Ctrl+K) é o centro.** Digitar `Jo 3:16`, `Graça`, `G26` ou `/sermão` leva ao destino.
3. **Estudar → Preparar → Pregar → Projetar** é um fluxo contínuo, não quatro programas.
4. **Leveza visível.** Abre em menos de 1 s; nunca trava para indexar (indexação em segundo plano com progresso).
5. **Conteúdo em primeiro lugar.** O texto bíblico é o protagonista: tipografia serifada, espaçamento generoso.
6. **Nunca perder trabalho.** Autosave constante.
7. **Respeito ao texto.** Palavras de Jesus, itálicos e notas do módulo preservados fielmente.

## 4. Funcionalidades por versão

### MVP (v0.1) — "Ler e estudar"
- Descobrir e listar módulos instalados (pasta do theWord ou pasta escolhida).
- Ler Bíblia por capítulo; trocar de tradução mantendo o versículo.
- Bíblias paralelas (até 4 colunas).
- Barra de comandos com referências em português (`Jo 3:16-18`, `1Co 13`, `Sl 23`).
- Clique em número de Strong → painel do léxico (módulo de dicionário).
- Comentários seguem o versículo selecionado.
- Referências cruzadas em chips clicáveis; passar o mouse mostra o texto.
- Busca por palavra/frase na tradução ativa e por número de Strong.
- Tema escuro e claro; tamanho de fonte ajustável.

### v0.2 — "Preparar o sermão"
- Editor de sermão em blocos: título, texto-base, pontos, ilustrações, aplicações.
- Inserir versículo digitando `/Jo 3:16` (escolhe tradução; mantém referência viva).
- Inserir palavra do léxico ou nota de comentário com um clique.
- Realces coloridos e anotações por versículo (salvos no banco do usuário).
- Exportar sermão: Markdown, PDF, DOCX.

### v0.3 — "Pregar e projetar"
- **Modo Púlpito:** tela limpa, fonte grande, texto bíblico + esboço lado a lado, cronômetro, navegação por teclado/pedal.
- **Modo Projeção:** segunda janela para o projetor com versículos e slides simples (fundo, fonte, rodapé com referência); controle remoto pela janela principal.
- Exportar esboço em slides (PPTX).

### v1.0
- Importar/exportar sermões e anotações (backup único).
- Sincronização opcional por pasta (Dropbox/Drive/OneDrive) sem servidor próprio.
- Plano de leitura e devocional diário (módulos `.dev.twm`).
- Instaladores assinados: Windows (MSI), macOS (dmg), Linux (AppImage/deb).

### Fora de escopo (por ora)
Conta de usuário, nuvem própria, IA generativa embutida, apócrifos/versificações alternativas, edição de módulos theWord.

## 5. Telas

1. **Estudo** (principal): biblioteca à esquerda · texto bíblico ao centro · painel direito com abas **Léxico / Comentário / Referências** e, abaixo, o **Sermão** em edição.
2. **Modo Púlpito:** duas colunas, sem barras laterais; texto grande à esquerda, esboço à direita, cronômetro na base.
3. **Projeção:** janela independente, só o conteúdo.
4. **Biblioteca:** gerenciar módulos (instalar por arrastar-e-soltar, ocultar, ver metadados).
5. **Configurações:** pastas de módulos, tema, fonte, idioma, atalhos.
6. **Primeiro uso:** localizar módulos → escolher Bíblia padrão → tour de 30 s.

## 6. Atalhos essenciais

| Atalho | Ação |
|---|---|
| `Ctrl+K` | Barra de comandos |
| `Ctrl+P` | Modo Púlpito |
| `Ctrl+Shift+P` | Janela de projeção |
| `Ctrl+←/→` | Capítulo anterior / seguinte |
| `Alt+1..4` | Alternar tradução paralela |
| `Ctrl+B` | Mostrar/ocultar biblioteca |
| `Ctrl+Enter` | Inserir seleção no sermão |
| `/` no editor | Menu de inserção (versículo, léxico, nota) |

## 7. Identidade visual (do protótipo original)

- Fundo `#0f1115` · painel `#161920` · cartão `#1e222d`
- Destaque âmbar `#d97706` / `#fbbf24` (reverente, quente)
- Chips de referência em azul `#60a5fa`
- Texto bíblico: fonte serifada (ex.: Source Serif 4); interface: sans-serif legível (ex.: IBM Plex Sans)
- Número de Strong: sublinhado tracejado âmbar; hover com fundo âmbar translúcido

## 8. Acessibilidade e idioma

- Contraste mínimo 4,5:1 para texto; foco visível; navegação completa por teclado.
- Leitores de tela: rótulos em todos os controles.
- Interface em pt-BR por padrão; cadeia de caracteres em i18n.
- Texto bíblico respeita direção do idioma do módulo (hebraico RTL).

## 9. Métricas de sucesso

- Tempo até abrir o primeiro versículo após instalar: **< 2 min**.
- Tempo para montar um esboço com 3 versículos inseridos: **< 5 min**.
- Abertura do app: **< 1 s**. RAM com 10 módulos: **< 80 MB** (excluindo webview do sistema).
- Zero perda de sermão em teste de queda de energia simulada.
