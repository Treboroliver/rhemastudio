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
- [ ] Palavras com Strong têm sublinhado tracejado; clicar ainda não faz nada (M4).
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
- [ ] Remover cada coluna pelo cabeçalho; alternar coluna principal com Alt+1..4; navegar e voltar/avançar no histórico mantém todas alinhadas.
- [ ] Versículo ausente deixa um espaço discreto; Bíblia sem o livro mostra aviso somente naquela coluna; módulo protegido não é adicionado e mantém a mensagem amigável.
- [ ] Em janela menor que 900 px, no máximo duas colunas são exibidas e há aviso; aumentar a janela restaura a visualização sem rolagem horizontal da página.
- [ ] Fechar e reabrir restaura a ordem das colunas escolhidas.
- [ ] Números de Strong não ficam sublinhados por padrão; hover e foco realçam a palavra. Ctrl+Shift+S alterna o sublinhado âmbar e persiste após reiniciar; clicar ainda não abre nada (M4).
- [ ] Ctrl+= aumenta, Ctrl+- diminui e Ctrl+0 restaura a fonte; os limites são 14 e 32 px e a preferência persiste após reiniciar.
- [ ] Alternar escuro/claro/automático aplica o tema sem recarregar; automático acompanha a preferência do sistema. Texto, avisos e destaques permanecem legíveis.
