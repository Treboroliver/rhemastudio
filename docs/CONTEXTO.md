# Contexto — M1.5

Atualizado em 2026-11-02. M1.5 concluído (RTF medido, parser RVF, título `<-`); M4b continua fora do escopo.

- RVF: `rvf.rs` converte as entradas de VidaNova (110), Strong_AMG2 (32, incl. G3056), BEP (1.181) para `Vec<Node>`; Holman tem 1 entrada RVF com dados NULL. Estrutura estranha vira `degraded` (aviso discreto na interface), nunca erro. Especificação em `docs/02-apendice-schemas.md`.
- RTF/espaços: a medição (2.000 entradas) mostrou que o conversor não descarta espaços reais; o "espaço perdido" é o delimitador após palavra de controle (RTF). Sem alteração no conversor.
- Biblioteca: valores de propriedade têm espaços e o sufixo ` <-` removidos (o marcador está no arquivo da BJ, em `lang` e `description`); `CATALOG_VERSION` = 5.
- Abertura `.twm` consulta só chaves selecionadas de `config`, nunca `about`.
- Comentários seguem paginados por até 60 parágrafos e 8.000 caracteres.
- Restrições observadas: conteúdo de módulos protegidos não foi aberto; nenhum texto real de módulo foi salvo em código, testes ou documentos.
