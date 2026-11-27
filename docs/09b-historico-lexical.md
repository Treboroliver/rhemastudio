# 09b — Histórico lexical (E1a.1–E1a.3)

> Seções movidas de [09-indice-lexical.md](09-indice-lexical.md) para manter o documento ativo enxuto.

## E1a.1 — Núcleo lexical e ligações
- Normalização: grego (NFD, sem marcas, minúsculas, sigma final → comum), hebraico (NFD, sem marcas/maqqef/direcionalidade, 5 letras finais), latim (NFD, sem marcas, minúsculas).
- Extração tolerante para Strong, Louw-Nida (domínio/entrada/subdomínio), DITAT (número + subentradas) e DITNT; sem persistir texto de verbete.
- Ligações SQLite (release, 4 módulos, 14.198 Strong, 1 protegido ignorado): varredura 94 ms, build 11.623 ms, 5.591.040 bytes; DITAT 6.301, Louw-Nida 7.067, DITNT 715.

## E1a.2 — Índice textual e busca (tantivy)
- Campos armazenados: `module_id`, `kind`, `key`; indexados: `key_norm`, `lemma`, `lemma_norm`, `translit`, `refs`, `body` (não armazenado). Troca atômica via `CURRENT` e invalidação por tamanho/data.
- Medições (release, 12 módulos legíveis, 1 protegido ignorado, 49.403 entradas): build 29.696 ms, 13,7 MB, abertura 2,56 ms, 200 consultas p50 0,13 ms / p95 1,53 ms.

## E1a.3 — HTML, desempate e total real
- `convert_html` decodifica `"html"` em `decode_twm_content_checked`; empate desempata por `strong > louw_nida > ditat > ditnt > generic`; `total` reflete `Count` real no tantivy.
- Medições (release): não lidas 5.442→0 (HTML) e 5.685→0 (texto simples); build 26.225 ms, 19,8 MB, abertura 2,47 ms, p50 0,14 ms / p95 1,50 ms.
