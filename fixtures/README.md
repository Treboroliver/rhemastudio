# Fixtures de Teste do RhemaStudio

Este diretório contém módulos sintéticos gerados para testes automatizados:

- `synthetic-commentary.cmt.twm`: referências de versículo, faixa, capítulo e livro, com conteúdo RTF sintético e uma entrada RVF não suportada.
- `synthetic-dictionary.dct.twm`: entradas RTF/RVF e índice de palavras sintéticos.
- `synthetic-encrypted.cmt.twm`: fixture com `do.encrypt=1`.
- `synthetic-secure.cmt.twm`: fixture com `secure=1`.

Gere-os com `cargo run -p gen-fixtures`. Não contêm conteúdo de módulos reais.
