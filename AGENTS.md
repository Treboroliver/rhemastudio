# AGENTS.md — RhemaStudio

> Leia este arquivo inteiro antes de escrever qualquer código. Ele é a fonte de verdade do projeto; os demais documentos detalham cada área.

## 1. Missão

Construir o **RhemaStudio**: um software de estudo e preparação de sermões, escrito em **Rust**, que **lê e exibe módulos do theWord** (Bíblias `.ont/.ot/.nt`, módulos `.twm` como comentários, dicionários e livros) de forma rápida e dinâmica.

Público: **pastores e pregadores**, não programadores. O critério de sucesso é: *um pastor instala, abre o app e prepara um sermão em menos de 10 minutos, sem ler manual.*

O produto não é uma cópia do theWord. O diferencial é o **fluxo completo: estudar → preparar → pregar → projetar**, com interface moderna, teclado em primeiro lugar e consumo mínimo de memória.

## 2. Ordem de leitura

1. `docs/01-produto-ux.md` — o que construir e como deve parecer
2. `docs/02-formato-theword.md` — formatos dos módulos (parsers)
3. `docs/03-arquitetura.md` — crates, IPC, dados, performance
4. `docs/04-roadmap.md` — milestones M0–M8 com critérios de aceite

Mockups de referência visual: protótipo HTML original do autor + canvas "RhemaStudio — Mockup da interface".

## 3. Regras inegociáveis

1. **Somente leitura nos módulos do usuário.** Nunca grave, renomeie ou altere arquivos de módulos theWord. Abra SQLite com `OpenFlags::SQLITE_OPEN_READ_ONLY`. Antes de consultar conteúdo de qualquer `.twm`, chame `rhema_twformat::ensure_twm_unprotected`; a checagem lê somente a tabela `config` e deve preceder qualquer acesso a `content`. A abertura de `.twm` na biblioteca ainda não existe; implementar essa chamada e cobri-la com fixture sintética protegida é requisito obrigatório da M1.4.
2. **Parsing tolerante.** Módulo malformado nunca derruba o app: retorne erro tipado, registre e siga. Zero `unwrap()`/`expect()` em código que toca arquivo do usuário.
3. **Não presuma o que não foi verificado.** Itens marcados `[VERIFICAR]` em `docs/02` devem ser confirmados contra módulos reais antes de virar código definitivo (ver tarefa M1.1). Se o schema real divergir do documento, **o schema real vence** — atualize o documento.
4. **Offline-first.** Nenhuma chamada de rede por padrão. Atualizações e downloads só com ação explícita do usuário.
5. **Nunca perder o trabalho do pastor.** Sermões são salvos continuamente (autosave a cada alteração, debounce 500 ms) e gravados de forma atômica (arquivo temporário + rename).
6. **Direitos autorais.** Não embuta módulos protegidos. Só distribua módulos cuja licença permita redistribuição (verifique cada um). Testes usam **fixtures sintéticas** geradas por script. Nunca tente descriptografar ou contornar proteção de módulos (`.ontx` e similares). Detecte pela assinatura ou pelas flags `do.encrypt=1`/`secure=1` em `.twm`, retorne o erro `EncryptedModule` e mostre uma mensagem amigável.
7. **Português do Brasil primeiro.** Todas as strings da interface ficam em arquivos de i18n (`pt-BR` padrão, `en` secundário). Abreviações de livros em português (`Jo`, `1Co`, `Sl`) e inglês.
8. **Teclado primeiro, mouse também.** Toda ação importante tem atalho; todo elemento interativo é focável e tem rótulo acessível.
9. **Orçamentos de performance** (seção 6 de `docs/03`) são requisito, não meta. CI falha se estourar.
10. **Não use nome/marca "theWord"** no produto. Texto de compatibilidade: "Compatível com módulos do theWord". O projeto não é afiliado ao theWord.

## 4. Stack

| Camada | Escolha | Motivo |
|---|---|---|
| Núcleo | Rust (edição 2021+, workspace Cargo) | Velocidade, segurança de memória, binário único |
| Shell desktop | **Tauri 2** | Instalador pequeno, RAM baixa, UI web moderna, Windows/Linux/macOS |
| Frontend | Svelte 5 + TypeScript + Tailwind | Leve, reativo, aproveita o protótipo em Tailwind |
| Dados do módulo | `rusqlite` (somente leitura) | `.twm` é SQLite |
| Busca | `tantivy` | Índice de texto completo local |
| Dados do usuário | SQLite próprio (`rhema.db`) + export Markdown | Robusto, portável |
| Erros | `thiserror` (libs), `anyhow` (app) | |
| Testes | `cargo test`, `insta` (snapshots), `proptest` (parsers), Playwright (UI) | |

Alternativa só se Tauri se mostrar inviável: Slint. Não troque sem registrar decisão em `docs/adr/`.

## 5. Estrutura do repositório

```
RhemaStudio/
├─ Cargo.toml               # workspace
├─ crates/
│  ├─ rhema-core/           # tipos: Book, VerseRef, VerseRange, RichText/Node, ModuleInfo
│  ├─ rhema-refs/           # parser de referências ("Jo 3:16-18", "1Co 13")
│  ├─ rhema-twformat/       # leitores .ont/.ot/.nt e .twm (SQLite)
│  ├─ rhema-library/        # descoberta, catálogo e cache de módulos
│  ├─ rhema-search/         # índice tantivy + busca por Strong
│  ├─ rhema-sermon/         # modelo de sermão, autosave, export
│  └─ rhema-cli/            # ferramenta de inspeção (twm-inspect, dump)
├─ app/
│  ├─ src-tauri/            # comandos IPC, janelas (principal e projeção)
│  └─ ui/                   # Svelte + Tailwind
├─ tools/gen-fixtures/      # gera módulos sintéticos para teste
├─ fixtures/
├─ docs/                    # estes documentos + adr/
└─ AGENTS.md
```

## 6. Comandos esperados

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p rhema-cli -- inspect <arquivo>
cargo tauri dev            # em app/
cargo tauri build
```

## 7. Definição de pronto (DoD) de cada tarefa

- Compila sem warnings; `fmt`, `clippy` e testes passam.
- Testes cobrem caminho feliz **e** entradas inválidas/vazias.
- Critério de aceite do milestone no `docs/04-roadmap.md` demonstrado (comando, teste ou captura de tela).
- Orçamentos de performance respeitados (benchmarks `criterion` onde aplicável).
- Documentação atualizada se algo mudou (principalmente `docs/02-formato-theword.md`).
- Mensagem de commit no formato Conventional Commits.

## 8. Como trabalhar

1. Pegue o próximo milestone não concluído em `docs/04-roadmap.md`.
2. Quebre em tarefas pequenas (≤ 1 dia de trabalho humano) e implemente em ordem.
3. Ao fim do milestone: rode a suíte completa, atualize o checklist em `docs/04-roadmap.md` e escreva um resumo curto em `docs/CHANGELOG.md`.
4. Registre decisões relevantes como ADR curto em `docs/adr/NNNN-titulo.md` (contexto, decisão, consequências).

## 9. Quando parar e perguntar ao humano

- Licença de um módulo ou fonte de dados incerta.
- Divergência entre o formato real do módulo e a especificação.
- Mudança que afete o escopo de um milestone ou a stack.
- Qualquer necessidade de rede, telemetria ou conta de usuário.

Em todos os outros casos: decida, registre e siga em frente.
