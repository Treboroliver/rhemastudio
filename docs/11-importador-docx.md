# 11 — Importador de livros pessoais (`.docx`)

Módulo `crates/rhema-import` + comando `rhema-cli import docx <arquivo> --kind dict|comm|book [--report]`.

## Leitura em fluxo e segurança

- Abre o pacote `.docx` (ZIP) em modo somente leitura, sem carregar o arquivo inteiro na memória.
- Lê `word/styles.xml`, `word/_rels/document.xml.rels` e faz streaming de `word/document.xml` com `quick-xml`.
- Ignora macros e nunca faz requisições de rede nem segue hiperlinks externos (apenas extrai o domínio/esquema e contabiliza).
- Limites rígidos por padrão (retornam erros tipados em `ImportError`, sem `panic`):
  - Entrada: até **5 MB** (`EntryTooLarge`).
  - Imagem em `word/media/`: até **20 MB** (`ImageTooLarge`), armazenando apenas metadados e tamanho.
  - Tabela: até **10.000 células** (`TableTooLarge`).
  - Descompressão por parte do ZIP: teto com `LimitedPartReader` (`ZipBomb`), interrompendo antes de alocar além do limite.

## Representação intermediária (IR)

```
Entry {
  id: usize,
  level: u8,
  title_path: Vec<String>,
  keys: Vec<EntryKey { kind: KeyType, value: String, lang: Option<String> }>,
  blocks: Vec<Block>,
  char_count: usize,
}
```

Blocos suportados em `Block`:
- `Paragraph { style_id, spans, anchors }` — trechos com `SpanStyle` (`bold`, `italic`, `underline`, `superscript`), campo ativo (`field`) e links (`Span::Link`).
- `ListItem { level, ordered, spans }` — itens de listas numeradas/com marcadores.
- `Image { target, content_type, size_bytes, alt }` — somente metadados e tamanho.
- `Table { rows: Vec<Vec<String>> }` — células em texto plano.

## Gramática de marcadores

Tolerante a espaços após `:` e a variações de maiúsculas/minúsculas (inclusive quando o Word divide um marcador em múltiplos `<w:r>`):

| Sintaxe | Papel |
|---|---|
| `[[@TIPO: VALOR]]` | Etiqueta: define chave da entrada ou âncora de parágrafo. |
| `[[TIPO: VALOR]]` | Link inline (também aceita `[[rótulo >> TIPO: VALOR]]`). |
| `{{field-on:NOME}}` / `{{field-off:NOME}}` | Delimita campo inline. Campo sem fechamento vale até o fim do parágrafo e é contabilizado. |

Tipos conhecidos (`KeyType`): `Headword` (aceita sufixo de idioma, ex.: `Headword+he`, `Headword+el`), `Bible` (`Bíblia`/`Biblia`), `HebrewStrongs`, `GreekStrongs`, `louw`, `TWOT`, `GreekGK`, `HebrewGK`. Tipos desconhecidos são preservados em `KeyType::Unknown(String)`, contabilizados e comparados por distância de edição (≤ 2) para apontar grafias suspeitas (ex.: `GreekStrong` → `GreekStrongs`).

## Regras por tipo de obra

- **Dicionário (`dict`)**: o nível de entrada padrão é o menor nível de título que contém etiquetas `Headword` ou `*Strongs` (configurável via `--entry-level`). Cada entrada vai até o próximo título do mesmo nível ou superior. Títulos acima formam apenas a hierarquia (`title_path`); títulos no nível de entrada sem etiqueta são contabilizados no relatório.
- **Comentário (`comm`)**: títulos com `[[@Bible: ...]]` iniciam seções por passagem; etiquetas `[[@Bible: ...]]` no início de parágrafo viram âncoras (`anchors`).
- **Livro (`book`)**: títulos dividem as seções preservando a hierarquia e eventuais chaves.

Referências bíblicas com ponto (`At 14.8-20`, `Mt 1.1-17`, `1Co 2.3`) ou dois-pontos são resolvidas com `rhema-refs` (pt-BR com fallback en). Referências não marcadas no corpo (ex.: `(Gn 31.28)`) são detectadas e apenas contadas nesta etapa.

## Relatório de validação

`ValidationReport` registra apenas contagens, chaves e exemplos de referências não resolvidas (até 10), nunca texto de verbete: entradas, hierarquia, chaves/etiquetas/links por tipo, `[[`/`]]` desbalanceados, campos sem fechamento, tipos suspeitos, headwords duplicados, Strongs duplicados e lacunas, entradas sem etiqueta, entradas vazias, referências resolvidas/não resolvidas/não marcadas, hiperlinks externos por domínio, imagens/tabelas/listas, maior verbete em caracteres, tempo e memória estimada.
