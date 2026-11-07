# E0 — Inventário exegético

Execução local em 07/11/2026: `rhema-cli inventory <pastas...>`, em `E:\coding\modulos-teste` e `C:\ProgramData\The Word`. Saída JSON contém as contagens completas e até dez amostras de chaves mascaradas por padrão; não contém texto de verbetes ou versículos. Tamanho em bytes; tempo de abertura em milissegundos, medido nesta execução.

## Módulos relevantes

| Título ou nome de arquivo | Estado | Idioma declarado | Bytes | Entradas | Abertura ms |
|---|---|---:|---:|---:|---:|
| Dicionario Champlin | pronto | não informado | 119186432 | 4734 | 4 |
| Dicionário Ilustrado da Bíblia | pronto | não informado | 20086784 | 5442 | 1 |
| DITATok | pronto | não informado | 28924928 | 3086 | 1 |
| Dicionário Internacional de Teologia do Novo Testamento | pronto | por | 27561984 | 360 | 2 |
| Enciclopédia temática da Bíblia | pronto | não informado | 24413184 | 1001 | 1 |
| Louw-Nida_Br | pronto | Por | 35667968 | 7071 | 2 |
| Mickelson's Strong (arquivo) | protegido | não lido | 15924224 | não lido | — |
| Novo Dicionário de Teologia | pronto | por | 22005760 | 851 | 1 |
| Strong Anotado Pela AMG | pronto | por | 36855808 | 14199 | 1 |
| NA28th-UBS5 | pronto | grc | 1718978 | 7957 | — |
| ARA+ | pronto | br | 7221554 | 31102 | — |
| KJV | pronto | eng | 7700651 | 31102 | — |
| ACF2011.ontx; acf2007.ontx | protegidos | não lido | 2313729; 4972187 | não lido | — |
| LTT2009.ontx; LTT2015.ontx | protegidos | não lido | 6396744; 7480718 | não lido | — |

As Bíblias protegidas são listadas por precaução, sem inferir idioma, presença de Strong ou quantidade de entradas. Nenhuma Bíblia hebraica, TR, WLC, BHS ou LXX foi identificada pelos metadados/títulos disponíveis; BDAG, Thayer, BDB, HALOT e TWOT independentes não foram identificados entre os arquivos encontrados.

## Dicionários prontos

Padrões de chaves são categorias exclusivas, em ordem: Strong, domínio decimal, número, grego, hebraico, outro/vazio. `orig` = quantidade preenchida de `strong_orig_word` (— = coluna ausente); `idx` = palavras distintas em `topics_wordindex` (— = tabela ausente). `NULL` usa o formato padrão indicado.

| Módulo | Chaves: contagem por padrão | orig / idx | `content_type` por linha; padrão |
|---|---|---:|---|
| Champlin | grego 1; outro 4733 | — / 7759 | rvf 4733, NULL 1; rvf |
| Ilustrado | outro 5442 | — / — | coluna ausente 5442; html |
| DITAT | hebraico 3084; outro 2 | — / 21734 | rtf 3084, rvf 2; rvf |
| DITNT | grego 309; outro 51 | — / 6510 | rtf 359, rvf 1; rtf |
| Enciclopédia temática | outro 1001 | — / 2415 | rvf 977, NULL 24; rvf |
| Louw-Nida | decimal 6978; número 93 | 0 / 7071 | rtf 7070, rvf 1; rtf |
| NDT | outro 851 | — / 2104 | rvf 850, NULL 1; rvf |
| Strong AMG | Strong 14198; outro 1 | 14198 / 14204 | rtf 14167, rvf 32; rtf |

Citações textuais, na ordem TWOT / LN / DITAT / G+número / H+número: Champlin 0/1/0/0/2; Ilustrado 0/0/0/24/1; DITAT 1/0/0/0/6329; DITNT 1/0/0/99/1; Enciclopédia 0/0/0/57/0; Louw-Nida 0/0/0/0/0; NDT 0/0/0/0/0; Strong AMG 0/0/18/5550/8755. São ocorrências, não pares comprovados de mapeamento. Faltaram conteúdos de 1 entrada em Champlin, 24 na Enciclopédia e 1 em NDT; suas contagens são limites inferiores. HTML é inspecionado como texto com tags removidas.

## Texto original, morfologia e versificação

- NA28th-UBS5: UTF-8, grego, direção LTR; nenhuma tag, WG, WH, WT, lema em tag ou vogal hebraica nos 7957 versículos. Não há códigos de morfologia para listar nem esquema identificável.
- ARA+ e KJV são as **duas Bíblias prontas com tags Strong**: respectivamente WG 114735 / WH 226246 e WG 122356 / WH 229274. Cada uma tem 31102 linhas; isso não comprova alinhamento dos 1189 capítulos.
- ARA+ apresenta tags numéricas candidatas a morfologia; dez mais frequentes mascaradas: x8799 19804, x8804 12486, x8802 5338, x8800 4846, x8686 4018, x5719 2989, x8798 2844, x8689 2651, x5723 2450, x8762 2436. O esquema e significado requerem verificação; não foram encontrados códigos WT na ARA+ ou KJV.
- Os arquivos sequenciais não fornecem limites de capítulo independentes que permitam verificar a correspondência por capítulo; o total canônico por si só não prova versificação alinhada.

## Respostas e limites

1. **Lema grego/hebraico:** há chaves gregas no DITNT, hebraicas no DITAT e 14198 campos originais no Strong AMG; esses dados permitem construir busca, mas a normalização e o índice ainda não foram implementados.
2. **Louw-Nida:** sim, por chaves de domínio decimal e numéricas, após implementar o índice; não foi comprovado mapa para Strong.
3. **TWOT/DITAT numérico:** não foi identificado dicionário TWOT independente; as chaves DITAT observadas são predominantemente hebraicas, não números. Citações H no DITAT e DITAT no Strong são pistas, não mapa validado.
4. **Morfologia:** apenas as tags numéricas candidatas da ARA+ foram observadas; esquema não confirmado. NA28th-UBS5 não apresenta tags de morfologia.
5. **Bloqueios:** Mickelson e as quatro Bíblias `.ontx` não foram abertas; idioma, entradas e capacidades não são conhecidos. Bibliotecas e esquemas ausentes exigem módulos disponíveis e verificação posterior.

O leitor existente indexa 7963 linhas para NA28th-UBS5: as seis finais são uma linha vazia e cinco comentários de configuração, excluídos do total do inventário. A saída da CLI registra ambos os totais.
