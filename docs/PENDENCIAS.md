# Pendências conhecidas

- Links RVF "interno", "arquivo" e "outro" do VidaNova não são clicáveis.
- Link `tw://dct.` do Strong_AMG2 está inerte.
- Sinais `<` e `>` da ARA+ (marca de texto disputado) aparecem como caracteres.
- M3.7: teste de fluxo completo (Playwright).
- Módulos `.gbk` e `.dev` não são suportados.
- Ctrl+B agora alterna o painel de estudo; a biblioteca ficou sem atalho.
- Ordem dos léxicos sem interface.
- Limite de 6 leitores abertos.
- Chips de referência cruzada (M4b) sem fonte de dados confirmada (tag `<RX>` nunca observada).
- Estilos de parágrafo (alinhamento) do RVF não aproveitados.
- Módulos protegidos (acf2007, LTT2009, LTT2015, Mickelson) nunca serão lidos, por política.
- Nome do produto (RhemaStudio) precisa de checagem de marca.
- Ctrl+Enter insere só um versículo (a faixa selecionada ainda não existe); "/Jo 3:16-18" usa a primeira faixa.
- Teste de 1.000 blocos da UI é lento e fica fora da suíte padrão (`RHEMA_SLOW_TESTS=1`).
- Busca lexical sem stemming: "amor" não encontra "amores" e "amores" não encontra "amor" (apenas correspondência sem acentos).
- Decodificador HTML (`convert_html`) extrai texto e parágrafos sem preservar formatação inline (negrito, itálico, links).
- Índice reverso palavra → Strong tokeniza apenas palavras latinas; Bíblias originais com Strong (`tr(gr).nt`, `tra.nt`, `tanakh+.ot`) não entram no índice reverso.
