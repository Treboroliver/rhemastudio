# Contexto — M4a.1

Atualizado em 2026-11-01. M4a.1 concluído; M4b e parser RVF continuam fora do escopo.

- RTF: o conversor descarta somente um espaço delimitador após palavra de controle; espaços de texto após escapes e grupos são preservados. Teste sintético cobre esses casos. A amostra mascarada de NICNT mostrou vírgula seguida diretamente por letras no RTF bruto; não foi inserido espaço nem alterado conteúdo.
- Contagens de vírgula-letra em 2.000 linhas: bruto→convertido — Holman-AT 0→0, NICNT 106→8.080 e Strong_AMG2 5→7 (1.998 linhas RTF). Sem correção de texto, os valores convertidos antes/depois permanecem iguais.
- Abertura `.twm` consulta só chaves selecionadas de `config`, nunca `about`; VidaNova: 47 ms no registro anterior e 2,596 ms de mediana em três aberturas release aquecidas.
- Comentários seguem paginados por até 60 parágrafos e 8.000 caracteres. Interface de colunas, menu `Aa` e preferência persistente de destaque de Jesus estão integrados; os tokens `--jesus` já passam contraste ≥ 4,5:1 nos dois temas.
- Restrições observadas: conteúdo de módulos protegidos não foi aberto; nenhum texto real de módulo foi salvo em código, testes ou documentos.
