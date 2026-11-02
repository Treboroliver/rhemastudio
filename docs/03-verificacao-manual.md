# Verificação manual (app real)

Marque cada item à mão ao testar com `cargo tauri dev` (em `app/`) e módulos reais. Anote o que falhar.

## Primeiro uso e biblioteca
- [ ] Na primeira abertura aparece o assistente de 3 passos; ao concluir abre a tela principal.
- [ ] Reabrir o app não mostra o assistente de novo.
- [ ] "Gerenciar pastas" abre o passo de localizar módulos; "Atualizar" refaz a varredura.

## Ctrl+K (teclado real)
- [ ] Ctrl+K abre a barra no topo com o foco no campo; Esc fecha e devolve o foco.
- [ ] Digitar "Jo 3:16-18" mostra a interpretação ao vivo e Enter vai à passagem.
- [ ] Digitar "jo" mostra sugestões de livros; ↑/↓ escolhem; Enter aplica.
- [ ] "Jo 99" mostra erro em linguagem simples ("João tem 21 capítulos").
- [ ] Tab não sai da janela enquanto ela está aberta.
- [ ] Digitar a abreviação de uma Bíblia da biblioteca troca a Bíblia ativa.

## Navegação entre livros
- [ ] Ctrl+→ em João 21 vai a Atos 1; Ctrl+← em Atos 1 volta a João 21.
- [ ] Em livro de um capítulo só (ex.: Judas), Ctrl+→ passa ao livro seguinte.
- [ ] Botões ‹ e › fazem o mesmo.
- [ ] Alt+← e Alt+→ percorrem o histórico (mais de 50 posições).

## Salmo 119
- [ ] Abrir Sl 119 é rápido e o texto aparece sem travar.
- [ ] Ctrl+K "Sl 119:150" monta o trecho, rola até o versículo 150 e o seleciona.
- [ ] Voltar (Alt+←) e avançar (Alt+→) até Sl 119:150 repete o mesmo resultado.
- [ ] ↓ no último versículo já visível continua para os seguintes; End vai ao 176.
- [ ] Leitor de tela (se disponível) encontra o texto dos 176 versículos.

## Troca de Bíblia
- [ ] Clicar em outra Bíblia pronta na árvore a torna ativa e recarrega o capítulo atual.
- [ ] Bíblia só com Novo Testamento pedindo Gênesis mostra "Esta Bíblia não contém Gênesis…".
- [ ] Capítulo com versículos faltando mostra os existentes e o aviso "Alguns versículos não existem nesta Bíblia".

## Módulo protegido
- [ ] Módulo protegido aparece com cadeado e "Módulo protegido: abra-o no theWord".
- [ ] Clicar nele não o define como Bíblia ativa e não mostra erro técnico.

## Leitura
- [ ] Palavras de Jesus aparecem em cor própria na KJV (ex.: João 3:3); a ARA+ não tem essa marcação.
- [ ] Nota de rodapé abre ao clicar e ao focar com o teclado; Esc fecha.
- [ ] Palavras com Strong não têm sublinhado por padrão; hover e foco realçam. Clicar (ou Enter com foco) abre a aba Léxico — ver "M4a" abaixo.
- [ ] Nenhum texto de marcação (como números de Strong) aparece no texto.

## Persistência
- [ ] Fechar e reabrir: a última passagem lida, a Bíblia ativa e as pastas continuam as mesmas.

## Teclado e acessibilidade
- [ ] Tab percorre toda a interface com foco visível.
- [ ] Setas navegam na árvore da biblioteca; Enter ativa.
- [ ] Setas ↑/↓ mudam o versículo selecionado.
- [ ] O texto secundário é legível (mínimo de 12 px) e o contraste é bom.

## M3b — Paralelas e aparência
- [ ] Adicionar uma Bíblia pelo botão `+` e por Ctrl+K digitando sua abreviação; não adicionar duplicata nem ultrapassar quatro.
- [ ] Cabeçalho fixo com uma célula por coluna (também com uma só), abreviação visível, nome completo na dica, marca ativa Alt+1..4 e remoção acessível.
- [ ] Botão `+ Bíblia` visível; na quarta coluna fica desabilitado e informa o limite na dica.
- [ ] Navegação anterior/título/próximo agrupados; menu `Aa` reúne fonte, tema, Strong e destaque de Jesus, com teclado acessível e texto ≥ 12 px.
- [ ] Remover cada coluna pelo cabeçalho; alternar coluna principal com Alt+1..4; navegar e voltar/avançar no histórico mantém todas alinhadas.
- [ ] Versículo ausente deixa um espaço discreto; Bíblia sem o livro mostra aviso somente naquela coluna; módulo protegido não é adicionado e mantém a mensagem amigável.
- [ ] Em janela menor que 900 px, no máximo duas colunas são exibidas e há aviso; aumentar a janela restaura a visualização sem rolagem horizontal da página.
- [ ] Fechar e reabrir restaura a ordem das colunas escolhidas.
- [ ] Números de Strong não ficam sublinhados por padrão; hover e foco realçam a palavra. Ctrl+Shift+S alterna o sublinhado âmbar e persiste após reiniciar; clicar abre o léxico (M4a), com o interruptor ligado ou desligado.
- [ ] Destaque das palavras de Jesus vem ligado, pode ser desligado no menu `Aa` e continua salvo após reiniciar; conferir contraste nos dois temas.
- [ ] Ctrl+= aumenta, Ctrl+- diminui e Ctrl+0 restaura a fonte; os limites são 14 e 32 px e a preferência persiste após reiniciar.
- [ ] Alternar escuro/claro/automático aplica o tema sem recarregar; automático acompanha a preferência do sistema. Texto, avisos e destaques permanecem legíveis.

## M4a — Léxico e comentários (com módulos reais, no escuro e no claro)
- [ ] O painel direito (420 px) aparece com as abas Léxico e Comentário; não há aba Referências. Ctrl+B (e o botão "Estudo" na barra superior) recolhe e reabre; fechar e reabrir o app mantém o estado.
- [ ] Clicar numa palavra com Strong (ex.: "amou" em Jo 3:16, numa Bíblia com Strong) abre o verbete correspondente (G25) com o foco no painel; a palavra fica destacada. Funciona com Ctrl+Shift+S ligado e desligado. Tab até a palavra + Enter faz o mesmo.
- [ ] Esc dentro do painel devolve o foco à palavra clicada.
- [ ] Palavra com mais de um Strong mostra a lista no topo; escolher outro número troca o verbete.
- [ ] Links dentro do verbete: para outro verbete (G/H) abre o verbete e o botão ‹ volta ao anterior; para um versículo navega a leitura e Alt+← volta à passagem anterior.
- [ ] Links de outros tipos aparecem como texto simples com dica ao pousar o mouse. Números gramaticais não são clicáveis.
- [ ] Sem nenhum dicionário na biblioteca: "Instale um dicionário para ver o léxico" e o botão "Gerenciar pastas" abre a localização de módulos. Verbete ausente: "Este número não está no dicionário escolhido". Verbete em RVF (ex.: G3056 no Strong_AMG2): aviso de formato não suportado.
- [ ] Dicionário protegido (Mickelson) nunca é usado nem listado como léxico.
- [ ] Aba Comentário: selecionar outro versículo atualiza a lista em instantes (≈ 150 ms); entradas agrupadas por módulo, do mais específico ao mais amplo, com "Versículo 16", "Capítulo 3", "Introdução ao livro" etc.
- [ ] Expandir uma entrada carrega o conteúdo; entradas longas mostram "Mostrar mais" e acrescentam a próxima página sem travar. Entrada RVF mostra "Formato ainda não suportado" só nela; as demais abrem normalmente.
- [ ] Comentário longo pagina por até 60 parágrafos e aproximadamente 8.000 caracteres; uma página não duplica nem perde texto entre parágrafos.
- [ ] Caixas de seleção "Comentários ativos": desmarcar um módulo some com suas entradas, marcar traz de volta; a escolha persiste ao reiniciar. Módulos protegidos não aparecem na lista.
- [ ] Sem comentários instalados: mensagem com "Gerenciar pastas". Sem módulos ativos: "Nenhum comentário ativo…". Versículo sem comentário: "Nenhum comentário para este versículo nos módulos ativos".
- [ ] Negrito, itálico, sublinhado, sobrescrito, parágrafos e links do RTF têm contraste legível nos dois temas; links de versículo/verbete se distinguem do texto.
- [ ] Abas: ←/→ (e Home/End) trocam de aba; o foco vai ao painel ao abrir; janelas abaixo de 1100 px mostram o painel sobre a leitura.
- [ ] Selecionar um versículo não pisca o capítulo nem tira o foco da palavra clicada.

## Sermão (M6b-1)
- [ ] Aba "Sermão": "Novo sermão" cria "Sermão sem título" e abre o editor com o foco no título.
- [ ] Escreva título, texto base e alguns blocos; aguarde o indicador "Salvo", feche o app, abra de novo e confira que tudo está salvo.
- [ ] Mover (botões e Alt+↑/↓), duplicar e remover blocos; "Bloco removido — Desfazer" some após 8 s.
- [ ] "Exportar" abre o diálogo nativo; salve, abra o arquivo .md e confira o conteúdo; cancelar mostra mensagem de cancelado.
- [ ] "Versões anteriores": Restaurar (com confirmação) recarrega o editor.
- [ ] Busca por título ignora acentos e maiúsculas; arquivar/desarquivar e excluir (com confirmação) funcionam.
- [ ] "Expandir" alterna 420 px ↔ 50% e persiste ao reiniciar; abaixo de 1100 px o painel cobre a leitura.
