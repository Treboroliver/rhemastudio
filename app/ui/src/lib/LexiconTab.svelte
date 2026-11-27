<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import RichContent from "./RichContent.svelte";
  import { asApiError } from "./reader";
  import {
    formatLexField,
    lexEntry,
    lexIndexBuild,
    lexIndexStatus,
    lexRelated,
    lexSearch,
    lookupStrong,
    morphDecode,
    strongOccurrences,
    strongTranslations,
    type LexEntryRequest,
    type LexIndexStatusDto,
    type LexProgressEvent,
    type LexRelatedDto,
    type LexRelatedItemDto,
    type LexSearchDto,
    type LexSearchHitDto,
    type LexiconEntryDto,
    type MorphDecodeDto,
    type StrongOccurrencesDto,
    type StrongRequest,
    type StrongTranslationsDto,
  } from "./study";

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string;
    books?: { number: number; abbreviation: string; name?: string }[];
    request: StrongRequest | null;
    entryRequest?: LexEntryRequest | null;
    active?: boolean;
    onVerse: (target: {
      book: number;
      chapter: number;
      verse: number;
      moduleId?: string;
    }) => void;
    onManageFolders: () => void;
    onInsert?: (key: string) => void;
    searchFn?: (query: string, limit?: number) => Promise<LexSearchDto>;
    statusFn?: () => Promise<LexIndexStatusDto>;
    buildFn?: () => Promise<LexIndexStatusDto>;
    loadEntryFn?: (
      moduleId: string,
      key: string,
      topicId?: number | null,
    ) => Promise<LexiconEntryDto>;
    loadRelatedFn?: (params: {
      strongId?: string | null;
      moduleId?: string | null;
      key?: string | null;
      topicId?: number | null;
    }) => Promise<LexRelatedDto>;
    occurrencesFn?: (
      strong: string,
      moduleId?: string | null,
      page?: number,
      pageSize?: number,
    ) => Promise<StrongOccurrencesDto>;
    translationsFn?: (
      strong: string,
      moduleId?: string | null,
      limit?: number
    ) => Promise<StrongTranslationsDto>
    morphDecodeFn?: (code: string, lang?: string | null) => Promise<MorphDecodeDto>
    searchDelay?: number
  }

  interface Visit {
    key: string;
    choices: string[];
    moduleId?: string;
    topicId?: number | null;
    morph?: string | null;
  }

  const HISTORY_LIMIT = 50;
  const listId = $props.id();
  const occSectionId = `${listId}-occurrences`;

  let {
    t,
    books = [],
    request,
    entryRequest = null,
    active = true,
    onVerse,
    onManageFolders,
    onInsert,
    searchFn = lexSearch,
    statusFn = lexIndexStatus,
    buildFn = lexIndexBuild,
    loadEntryFn = lexEntry,
    loadRelatedFn = lexRelated,
    occurrencesFn = strongOccurrences,
    translationsFn = strongTranslations,
    morphDecodeFn = morphDecode,
    searchDelay = 120,
  }: Props = $props();

  let visits = $state<Visit[]>([]);
  let entry = $state<LexiconEntryDto | null>(null);
  let primaryEntry = $state<LexiconEntryDto | null>(null);
  let relatedItems = $state<LexRelatedItemDto[]>([]);
  let relatedOpen = $state(false);
  let relatedActive = $state(0);
  let preferredModuleId = $state<string | null>(null);
  let errorCode = $state<string | null>(null);
  let loading = $state(false);
  let sequence = 0;

  let searchQuery = $state("");
  let searchResult = $state<LexSearchDto | null>(null);
  let searchErrorCode = $state<string | null>(null);
  let searchOpen = $state(false);
  let activeHit = $state(0);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let searchSeq = 0;

  let indexStatus = $state<LexIndexStatusDto | null>(null);
  let unlisten: UnlistenFn | null = null;

  let occOpen = $state(true);
  let occLoading = $state(false);
  let occErrorCode = $state<string | null>(null);
  let occData = $state<StrongOccurrencesDto | null>(null);
  let occTranslations = $state<StrongTranslationsDto | null>(null);
  let occSelectedBible = $state<string | null>(null);
  let occPage = $state(1);
  let occSeq = 0;

  let morphResult = $state<MorphDecodeDto | null>(null)
  let morphSeq = 0

  const current = $derived(visits.length ? visits[visits.length - 1] : null);

  const activeStrong = $derived.by<string | null>(() => {
    const candidates = [current?.key, primaryEntry?.key, entry?.key];
    for (const c of candidates) {
      if (c && /^[GH]\d{1,5}$/i.test(c.trim())) {
        return c.trim().toUpperCase();
      }
    }
    return null;
  });

  const maxBookOcc = $derived.by(() => {
    if (!occData?.by_book.length) return 1;
    return Math.max(1, ...occData.by_book.map((b) => b.occurrences));
  });

  function bookLabel(bookNum: number): string {
    const found = books.find((b) => b.number === bookNum);
    return found?.abbreviation ?? `#${bookNum}`;
  }

  function langLabel(lang: string): string {
    const key = `occLang_${lang}`;
    const translated = t(key);
    return translated === key ? lang.toUpperCase() : translated;
  }

  async function loadOccurrences(
    strong: string,
    moduleId: string | null,
    page: number,
  ) {
    const hasTauri =
      typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!hasTauri && occurrencesFn === strongOccurrences) return;
    const id = ++occSeq;
    occLoading = true;
    occErrorCode = null;
    try {
      const res = await occurrencesFn(strong, moduleId, page, 50);
      if (id !== occSeq) return;
      occData = res;
      occSelectedBible = res.selected_module_id;
      occPage = res.page;
      if (res.selected_module_id && res.total_occurrences > 0) {
        try {
          const tr = await translationsFn(strong, res.selected_module_id, 12);
          if (id === occSeq) occTranslations = tr;
        } catch {
          if (id === occSeq) occTranslations = null;
        }
      } else {
        occTranslations = null;
      }
    } catch (cause) {
      if (id !== occSeq) return;
      occData = null;
      occTranslations = null;
      occErrorCode = asApiError(cause).code;
    } finally {
      if (id === occSeq) occLoading = false;
    }
  }

  function onSelectOccBible(event: Event) {
    const mid = (event.currentTarget as HTMLSelectElement).value || null;
    occSelectedBible = mid;
    occPage = 1;
    if (activeStrong) {
      void loadOccurrences(activeStrong, mid, 1);
    }
  }

  function goOccPage(nextPage: number) {
    if (!activeStrong || !occData) return;
    if (nextPage < 1 || nextPage > occData.total_pages) return;
    occPage = nextPage;
    void loadOccurrences(activeStrong, occSelectedBible, nextPage);
  }

  const flatHits = $derived.by<LexSearchHitDto[]>(() => {
    if (!searchResult) return [];
    return searchResult.groups.flatMap((g) => g.hits);
  });

  const statusText = $derived.by(() => {
    if (!indexStatus) return "";
    switch (indexStatus.state) {
      case "ready":
        return t("lexIndexReady");
      case "building":
        return t("lexIndexBuilding", { percent: indexStatus.percent });
      case "rebuild_needed":
        return t("lexIndexRebuildNeeded");
      case "empty":
        return t("lexIndexEmpty");
      default:
        return t("lexIndexError");
    }
  });

  async function refreshStatus() {
    try {
      indexStatus = await statusFn();
    } catch {
      // Em testes unitários sem backend Tauri, mantém silencioso
    }
  }

  async function rebuildIndex() {
    try {
      indexStatus = {
        state: "building",
        stage: "links",
        module_index: 0,
        module_count: 1,
        percent: 0,
        module_id: null,
        entries_processed: 0,
        message: null,
      };
      indexStatus = await buildFn();
    } catch (cause) {
      indexStatus = {
        state: "error",
        stage: null,
        module_index: 0,
        module_count: 0,
        percent: 0,
        module_id: null,
        entries_processed: 0,
        message: asApiError(cause).message,
      };
    }
  }

  async function load(key: string, moduleId?: string, topicId?: number | null) {
    const id = ++sequence;
    loading = true;
    entry = null;
    primaryEntry = null;
    relatedItems = [];
    relatedOpen = false;
    errorCode = null;
    const hasTauri =
      typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    const useRelated = hasTauri || loadRelatedFn !== lexRelated;
    try {
      if (useRelated) {
        let res: LexRelatedDto | undefined;
        try {
          res = await loadRelatedFn(
            moduleId ? { moduleId, key, topicId } : { strongId: key },
          );
        } catch (cause) {
          if (loadRelatedFn !== lexRelated) throw cause;
        }
        if (id !== sequence) return;
        if (res?.primary) {
          primaryEntry = res.primary;
          relatedItems = res.related;
          const preferred = preferredModuleId
            ? res.related.find(
                (r) => r.module_id === preferredModuleId && !r.error,
              )
            : undefined;
          if (preferred) {
            const switched = await loadEntryFn(
              preferred.module_id,
              preferred.key,
              preferred.topic_id,
            );
            if (id !== sequence) return;
            entry = switched;
          } else {
            entry = res.primary;
          }
          return;
        }
      }
      const found = moduleId
        ? await loadEntryFn(moduleId, key, topicId)
        : await lookupStrong(key);
      if (id !== sequence) return;
      entry = found;
    } catch (cause) {
      if (id !== sequence) return;
      errorCode = asApiError(cause).code;
    } finally {
      if (id === sequence) loading = false;
    }
  }

  async function selectRelated(item: LexRelatedItemDto | null) {
    relatedOpen = false;
    if (!item) {
      if (primaryEntry) {
        preferredModuleId = primaryEntry.module_id;
        entry = primaryEntry;
      }
      return;
    }
    if (item.error) return;
    preferredModuleId = item.module_id;
    const id = ++sequence;
    loading = true;
    errorCode = null;
    try {
      const found = await loadEntryFn(item.module_id, item.key, item.topic_id);
      if (id !== sequence) return;
      entry = found;
    } catch (cause) {
      if (id !== sequence) return;
      errorCode = asApiError(cause).code;
    } finally {
      if (id === sequence) loading = false;
    }
  }

  function onRelatedKeydown(event: KeyboardEvent) {
    const total = relatedItems.length + (primaryEntry ? 1 : 0);
    if (!total) return;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      relatedOpen = false;
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      relatedOpen = true;
      relatedActive = (relatedActive + 1) % total;
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      relatedOpen = true;
      relatedActive = (relatedActive - 1 + total) % total;
    } else if (event.key === "Enter" && relatedOpen) {
      event.preventDefault();
      if (primaryEntry && relatedActive === 0) {
        void selectRelated(null);
      } else {
        const idx = primaryEntry ? relatedActive - 1 : relatedActive;
        const item = relatedItems[idx];
        if (item) void selectRelated(item);
      }
    }
  }

  function visit(
    key: string,
    choices: string[],
    moduleId?: string,
    topicId?: number | null,
    morph?: string | null,
  ) {
    if (
      current?.key === key &&
      current?.moduleId === moduleId &&
      current?.topicId === topicId &&
      current?.morph === morph
    ) {
      visits = [
        ...visits.slice(0, -1),
        { key, choices, moduleId, topicId, morph },
      ];
      return void load(key, moduleId, topicId);
    }
    visits = [...visits, { key, choices, moduleId, topicId, morph }].slice(
      -HISTORY_LIMIT,
    );
    void load(key, moduleId, topicId);
  }

  function choose(key: string) {
    if (!current || current.key === key) return;
    visits = [...visits.slice(0, -1), { key, choices: current.choices }];
    void load(key);
  }

  function back() {
    if (visits.length < 2) return;
    visits = visits.slice(0, -1);
    const prev = visits[visits.length - 1];
    void load(prev.key, prev.moduleId, prev.topicId);
  }

  function openHit(hit: LexSearchHitDto) {
    searchOpen = false;
    if (hit.kind === "strong" || /^[GH]\d+$/i.test(hit.key)) {
      visit(hit.key, [hit.key]);
    } else {
      visit(hit.key, [hit.key], hit.module_id, hit.topic_id);
    }
  }

  async function runSearch(q: string) {
    const id = ++searchSeq;
    const trimmed = q.trim();
    if (!trimmed) {
      searchResult = null;
      searchErrorCode = null;
      searchOpen = false;
      return;
    }
    try {
      const res = await searchFn(trimmed, 30);
      if (id !== searchSeq) return;
      searchResult = res;
      searchErrorCode = null;
      searchOpen = true;
      activeHit = 0;
    } catch (cause) {
      if (id !== searchSeq) return;
      searchResult = null;
      searchErrorCode = asApiError(cause).code;
      searchOpen = true;
    }
  }

  function onSearchInput(event: Event) {
    clearTimeout(searchTimer);
    const q =
      (event.currentTarget as HTMLInputElement | null)?.value ??
      searchQuery ??
      "";
    searchQuery = q;
    if (!q.trim()) {
      searchResult = null;
      searchErrorCode = null;
      searchOpen = false;
      return;
    }
    searchTimer = setTimeout(() => void runSearch(q), searchDelay);
  }

  function onSearchKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      if (searchOpen) {
        event.preventDefault();
        event.stopPropagation();
        searchOpen = false;
      }
    } else if (event.key === "ArrowDown" && flatHits.length) {
      event.preventDefault();
      searchOpen = true;
      activeHit = (activeHit + 1) % flatHits.length;
    } else if (event.key === "ArrowUp" && flatHits.length) {
      event.preventDefault();
      searchOpen = true;
      activeHit = (activeHit - 1 + flatHits.length) % flatHits.length;
    } else if (event.key === "Enter" && flatHits.length && searchOpen) {
      event.preventDefault();
      const hit = flatHits[Math.min(activeHit, flatHits.length - 1)];
      if (hit) openHit(hit);
    }
  }

  function hitIndex(hit: LexSearchHitDto): number {
    return flatHits.indexOf(hit);
  }

  $effect(() => {
    const incoming = request;
    if (!incoming || !incoming.ids.length) return;
    untrack(() =>
      visit(
        incoming.ids[0],
        incoming.ids,
        undefined,
        undefined,
        incoming.morph ?? null,
      ),
    );
  });

  $effect(() => {
    const code = current?.morph ?? null;
    if (!code) {
      morphResult = null;
      return;
    }
    const hasTauri =
      typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!hasTauri && morphDecodeFn === morphDecode) {
      morphResult = {
        code,
        found: false,
        explanation: code,
        module_id: null,
        module_title: null,
      };
      return;
    }
    const id = ++morphSeq;
    void morphDecodeFn(code)
      .then((res) => {
        if (id === morphSeq) morphResult = res;
      })
      .catch(() => {
        if (id === morphSeq) {
          morphResult = {
            code,
            found: false,
            explanation: code,
            module_id: null,
            module_title: null,
          };
        }
      });
  });

  $effect(() => {
    const incoming = entryRequest;
    if (!incoming) return;
    untrack(() =>
      visit(incoming.key, [incoming.key], incoming.moduleId, incoming.topicId),
    );
  });

  $effect(() => {
    const strong = activeStrong;
    const isOpen = occOpen;
    if (!strong || !isOpen) {
      if (!strong) {
        occData = null;
        occTranslations = null;
        occErrorCode = null;
      }
      return;
    }
    untrack(() => {
      occPage = 1;
      void loadOccurrences(strong, occSelectedBible, 1);
    });
  });

  onMount(() => {
    const hasTauri =
      typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (active && (hasTauri || statusFn !== lexIndexStatus)) {
      void refreshStatus();
    }
    if (!hasTauri) return;
    void (async () => {
      try {
        unlisten = await listen<LexProgressEvent>(
          "lex-index-progress",
          (ev) => {
            const p = ev.payload;
            indexStatus = {
              state: p.state as LexIndexStatusDto["state"],
              stage: p.stage,
              module_index: p.module_index,
              module_count: p.module_count,
              percent: p.percent,
              module_id: p.module_id,
              entries_processed: p.entries_processed,
              message: null,
            };
            if (p.state === "ready" && occOpen && activeStrong) {
              void loadOccurrences(activeStrong, occSelectedBible, 1);
            }
          },
        );
      } catch {
        // Sem runtime Tauri (ex.: vitest)
      }
    })();
  });

  onDestroy(() => {
    clearTimeout(searchTimer);
    unlisten?.();
  });

  const actions = {
    onVerse: (target: { book: number; chapter: number; verse: number }) =>
      onVerse(target),
    onEntry: (id: string) => visit(id, [id]),
  };
</script>

<div class="lexicon-tab">
  <div class="lex-toolbar">
    {#if indexStatus}
      <div class="lex-index-status" role="status">
        <span>{statusText}</span>
        <button
          type="button"
          class="quiet compact-control"
          disabled={indexStatus.state === "building"}
          onclick={() => void rebuildIndex()}>{t("lexRebuildButton")}</button
        >
      </div>
    {/if}
    <div class="lex-search-box">
      <input
        type="search"
        class="lex-search-input"
        bind:value={searchQuery}
        oninput={onSearchInput}
        onkeydown={onSearchKeydown}
        role="combobox"
        aria-label={t("lexSearchLabel")}
        aria-expanded={searchOpen && flatHits.length > 0}
        aria-controls={listId}
        aria-activedescendant={searchOpen && flatHits.length
          ? `${listId}-${activeHit}`
          : undefined}
        placeholder={t("lexSearchPlaceholder")}
        autocomplete="off"
        spellcheck="false"
      />
      {#if searchOpen}
        <div
          class="lex-search-results"
          id={listId}
          role="listbox"
          aria-label={t("lexSearchLabel")}
        >
          {#if searchErrorCode === "lex_index_building"}
            <p class="panel-empty" role="status">{t("lexSearchBuilding")}</p>
          {:else if searchErrorCode === "lex_rebuild_required"}
            <p class="panel-empty" role="status">
              {t("lexIndexRebuildNeeded")}
            </p>
          {:else if searchErrorCode}
            <p class="panel-empty" role="alert">{t("lexSearchError")}</p>
          {:else if searchResult && searchResult.groups.length === 0}
            <p class="panel-empty" role="status">{t("lexSearchEmpty")}</p>
          {:else if searchResult}
            {#each searchResult.groups as group (group.label)}
              <div class="lex-group">
                <p class="lex-group-title">
                  {group.is_suggestions
                    ? t("lexSuggestionsGroup")
                    : group.label}
                </p>
                {#if group.is_suggestions}
                  {@const greek = group.hits.filter((h) =>
                    h.key.startsWith("G"),
                  )}
                  {@const hebrew = group.hits.filter((h) =>
                    h.key.startsWith("H"),
                  )}
                  <div class="lex-suggestions-split">
                    {#if greek.length}
                      <div class="lex-sug-col">
                        <span class="lex-sug-lang">{t("lexGreekGroup")}</span>
                        {#each greek as hit (hit.key)}
                          {@const idx = hitIndex(hit)}
                          <button
                            type="button"
                            id={`${listId}-${idx}`}
                            role="option"
                            aria-selected={idx === activeHit}
                            class="lex-hit"
                            class:active={idx === activeHit}
                            onclick={() => openHit(hit)}
                          >
                            <span class="lex-hit-key">{hit.key}</span>
                            {#if hit.dice !== null}
                              <span class="lex-hit-dice"
                                >{t("lexDice", {
                                  value: hit.dice.toFixed(2),
                                })}</span
                              >
                            {/if}
                          </button>
                        {/each}
                      </div>
                    {/if}
                    {#if hebrew.length}
                      <div class="lex-sug-col">
                        <span class="lex-sug-lang">{t("lexHebrewGroup")}</span>
                        {#each hebrew as hit (hit.key)}
                          {@const idx = hitIndex(hit)}
                          <button
                            type="button"
                            id={`${listId}-${idx}`}
                            role="option"
                            aria-selected={idx === activeHit}
                            class="lex-hit"
                            class:active={idx === activeHit}
                            onclick={() => openHit(hit)}
                          >
                            <span class="lex-hit-key">{hit.key}</span>
                            {#if hit.dice !== null}
                              <span class="lex-hit-dice"
                                >{t("lexDice", {
                                  value: hit.dice.toFixed(2),
                                })}</span
                              >
                            {/if}
                          </button>
                        {/each}
                      </div>
                    {/if}
                  </div>
                {:else}
                  {#each group.hits as hit (`${hit.module_id}-${hit.key}-${hit.field}`)}
                    {@const idx = hitIndex(hit)}
                    <button
                      type="button"
                      id={`${listId}-${idx}`}
                      role="option"
                      aria-selected={idx === activeHit}
                      class="lex-hit"
                      class:active={idx === activeHit}
                      onclick={() => openHit(hit)}
                    >
                      <span class="lex-hit-key">{hit.key}</span>
                      <span class="lex-hit-meta">
                        {formatLexField(t, hit.field)}{hit.provenance
                          ? ` · ${hit.provenance}`
                          : ""}
                      </span>
                    </button>
                  {/each}
                {/if}
              </div>
            {/each}
          {/if}
        </div>
      {/if}
    </div>
  </div>

  {#if !current}
    <p class="panel-empty" role="status">{t("lexiconHint")}</p>
  {:else}
    <div class="lexicon-head">
      <button
        type="button"
        class="quiet compact-control"
        aria-label={t("lexiconBack")}
        title={t("lexiconBack")}
        disabled={visits.length < 2}
        onclick={back}>‹</button
      >
      {#if current.choices.length > 1}
        <div
          class="strong-choices"
          role="group"
          aria-label={t("lexiconChoices")}
        >
          {#each current.choices as choice (choice)}
            <button
              type="button"
              class="strong-choice"
              aria-pressed={choice === current.key}
              onclick={() => choose(choice)}>{choice}</button
            >
          {/each}
        </div>
      {:else}
        <span class="strong-key">{current.key}</span>
      {/if}
    </div>
    {#if current.choices.length > 1}
      <p class="panel-note">{t("lexiconChooseNote")}</p>
    {/if}

    {#if loading}
      <p class="panel-empty" role="status">{t("lexiconLoading")}</p>
    {:else if entry}
      {#if relatedItems.length > 0}
        <div class="lex-related-bar">
          <button
            type="button"
            class="quiet compact-control lex-related-toggle"
            aria-expanded={relatedOpen}
            aria-haspopup="listbox"
            aria-label={t("lexOtherDictionariesLabel")}
            onclick={() => {
              relatedOpen = !relatedOpen;
              relatedActive = 0;
            }}
            onkeydown={onRelatedKeydown}
            >{t("lexOtherDictionaries", { count: relatedItems.length })} ▾</button
          >
          {#if relatedOpen}
            <ul
              class="lex-related-menu"
              role="listbox"
              aria-label={t("lexOtherDictionariesLabel")}
            >
              {#if primaryEntry}
                <li>
                  <button
                    type="button"
                    role="option"
                    aria-selected={entry.module_id === primaryEntry.module_id &&
                      entry.key === primaryEntry.key}
                    class="lex-hit"
                    class:active={relatedActive === 0}
                    onclick={() => void selectRelated(null)}
                  >
                    <span class="lex-hit-key">{primaryEntry.module_title}</span>
                    <span class="lex-related-reason"
                      >{t("lexPrimaryEntry")}</span
                    >
                  </button>
                </li>
              {/if}
              {#each relatedItems as item, idx (`${item.module_id}-${item.key}`)}
                {@const optIdx = primaryEntry ? idx + 1 : idx}
                <li>
                  <button
                    type="button"
                    role="option"
                    aria-selected={entry.module_id === item.module_id &&
                      entry.key === item.key}
                    disabled={!!item.error}
                    class="lex-hit"
                    class:active={relatedActive === optIdx}
                    onclick={() => void selectRelated(item)}
                  >
                    <span class="lex-hit-key"
                      >{item.module_title} · {item.title}</span
                    >
                    <span class="lex-related-reason"
                      >{item.error ?? item.reason}</span
                    >
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/if}
      <article
        class="lexicon-entry"
        aria-label={t("lexiconEntryLabel", { key: entry.key })}
      >
        <header>
          <h3>{entry.title}</h3>
          {#if entry.original_word}<p class="original-word" dir="auto">
              {entry.original_word}
            </p>{/if}
          <p class="panel-note">{entry.module_title}</p>
          {#if onInsert}
            <button
              type="button"
              class="compact-control"
              data-focus="insert-lexicon"
              onclick={() => onInsert(entry!.key)}
              >{t("insertIntoSermon")}</button
            >
          {/if}
        </header>
        {#if current?.morph}
          <section class="lex-morph-card" aria-label={t("morphTitle")}>
            <div class="lex-morph-head">
              <span class="lex-morph-label">{t("morphTitle")}</span>
              <code class="lex-morph-code">{current.morph}</code>
            </div>
            {#if morphResult}
              <p class="lex-morph-desc">{morphResult.explanation}</p>
              {#if morphResult.module_title}
                <p class="panel-note">{morphResult.module_title}</p>
              {/if}
            {/if}
          </section>
        {/if}
        {#if entry.degraded}<p class="panel-note" role="status">
            {t("contentDegraded")}
          </p>{/if}
        <RichContent nodes={entry.nodes} {t} {actions} />
      </article>

      {#if activeStrong}
        <section class="lex-occurrences" aria-label={t("occTitle")}>
          <button
            type="button"
            class="occ-toggle"
            aria-expanded={occOpen}
            aria-controls={occSectionId}
            onclick={() => (occOpen = !occOpen)}
          >
            <span>{t("occTitle")}</span>
            <span class="occ-toggle-meta">
              {#if occData && occData.total_occurrences > 0}
                {occData.total_occurrences}
              {/if}
              {occOpen ? "▾" : "▸"}
            </span>
          </button>

          {#if occOpen}
            <div class="occ-body" id={occSectionId}>
              {#if indexStatus?.state === "building" || occErrorCode === "lex_index_building"}
                <p class="panel-empty" role="status">{t("occBuilding")}</p>
              {:else if occLoading}
                <p class="panel-empty" role="status">{t("occLoading")}</p>
              {:else if occErrorCode}
                <p class="panel-empty" role="alert">{t("occError")}</p>
              {:else if occData}
                {#if occData.bibles.length === 0}
                  <p class="panel-empty" role="status">
                    {t("occNoStrongBible")}
                  </p>
                {:else}
                  <div class="occ-controls">
                    <label class="occ-bible-label">
                      <span>{t("occBibleLabel")}</span>
                      <select
                        class="occ-bible-select"
                        aria-label={t("occBibleLabel")}
                        value={occSelectedBible ?? ""}
                        onchange={onSelectOccBible}
                      >
                        {#each occData.bibles as bible (bible.module_id)}
                          <option value={bible.module_id}>
                            {bible.abbreviation ?? bible.title} ({langLabel(
                              bible.lang,
                            )}) · {bible.total_occurrences}
                          </option>
                        {/each}
                      </select>
                    </label>
                    <p class="occ-summary" role="status">
                      {t("occSummary", {
                        verses: occData.total_verses,
                        occurrences: occData.total_occurrences,
                      })}
                    </p>
                  </div>

                  {#if occData.total_occurrences === 0}
                    <p class="panel-empty" role="status">
                      {t("occEmpty", { strong: occData.strong })}
                    </p>
                  {:else}
                    {#if occTranslations && occTranslations.items.length > 0}
                      <div class="occ-block">
                        <h4 class="occ-subtitle">{t("occTranslations")}</h4>
                        <div class="occ-chips" role="list">
                          {#each occTranslations.items as item (item.word)}
                            <span
                              class="occ-chip"
                              role="listitem"
                              title={`${item.percent.toFixed(1)}%`}
                            >
                              {item.word} <strong>{item.n}</strong>
                            </span>
                          {/each}
                        </div>
                      </div>
                    {/if}

                    {#if occData.by_book.length > 0}
                      <div class="occ-block">
                        <h4 class="occ-subtitle">{t("occByBook")}</h4>
                        <ul class="occ-books">
                          {#each occData.by_book as b (b.book)}
                            <li class="occ-book-row">
                              <span class="occ-book-name"
                                >{bookLabel(b.book)}</span
                              >
                              <span class="occ-bar-track" aria-hidden="true">
                                <span
                                  class="occ-bar-fill"
                                  style={`width: ${Math.max(4, Math.round((b.occurrences / maxBookOcc) * 100))}%`}
                                ></span>
                              </span>
                              <span class="occ-book-count">{b.occurrences}</span
                              >
                            </li>
                          {/each}
                        </ul>
                      </div>
                    {/if}

                    <div class="occ-block">
                      <h4 class="occ-subtitle">{t("occReferences")}</h4>
                      <ul class="occ-refs">
                        {#each occData.items as item (`${item.book}-${item.chapter}-${item.verse}`)}
                          <li>
                            <button
                              type="button"
                              class="occ-ref-btn"
                              onclick={() =>
                                onVerse({
                                  book: item.book,
                                  chapter: item.chapter,
                                  verse: item.verse,
                                  moduleId: occSelectedBible ?? undefined,
                                })}
                            >
                              {`${bookLabel(item.book)} ${item.chapter}:${item.verse}${item.n > 1 ? ` (${item.n}×)` : ""}`}
                            </button>
                          </li>
                        {/each}
                      </ul>
                      {#if occData.total_pages > 1}
                        <div class="occ-pagination">
                          <button
                            type="button"
                            class="quiet compact-control"
                            disabled={occPage <= 1}
                            onclick={() => goOccPage(occPage - 1)}
                            >{t("occPrevPage")}</button
                          >
                          <span class="occ-page-info">
                            {t("occPage", {
                              page: occPage,
                              total: occData.total_pages,
                            })}
                          </span>
                          <button
                            type="button"
                            class="quiet compact-control"
                            disabled={occPage >= occData.total_pages}
                            onclick={() => goOccPage(occPage + 1)}
                            >{t("occNextPage")}</button
                          >
                        </div>
                      {/if}
                    </div>
                  {/if}
                {/if}
              {/if}
            </div>
          {/if}
        </section>
      {/if}
    {:else if errorCode === "no_lexicon_installed"}
      <div class="panel-empty" role="status">
        <p>{t("lexiconNone")}</p>
        <button type="button" class="primary" onclick={onManageFolders}
          >{t("manageFolders")}</button
        >
      </div>
    {:else if errorCode === "entry_not_found"}
      <p class="panel-empty" role="status">
        {current.moduleId ? t("lexiconEntryNotFound") : t("lexiconNotFound")}
      </p>
    {:else if errorCode === "entry_empty"}
      <p class="panel-empty" role="status">{t("lexiconEntryEmpty")}</p>
    {:else if errorCode === "unsupported_format"}
      <p class="panel-empty" role="status">{t("lexiconUnsupported")}</p>
    {:else if errorCode === "module_not_found"}
      <p class="panel-empty" role="status">{t("lexiconModuleMissing")}</p>
    {:else if errorCode === "module_protected"}
      <p class="panel-empty" role="status">{t("lexiconModuleProtected")}</p>
    {:else if errorCode === "module_corrupted"}
      <p class="panel-empty" role="status">{t("lexiconCorrupted")}</p>
    {:else if errorCode}
      <div class="panel-empty" role="alert">
        <p>{t("lexiconError")}</p>
        <button
          type="button"
          class="quiet"
          onclick={() =>
            current && load(current.key, current.moduleId, current.topicId)}
          >{t("retry")}</button
        >
      </div>
    {/if}
  {/if}
</div>

<style>
  .lex-morph-card {
    margin: 0.5rem 0 0.75rem;
    padding: 0.5rem 0.65rem;
    border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
    border-radius: 6px;
    background: var(--surface, rgba(128, 128, 128, 0.08));
  }

  .lex-morph-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    margin-bottom: 0.25rem;
  }

  .lex-morph-label {
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.75;
  }

  .lex-morph-code {
    font-size: 0.85rem;
    font-weight: 600;
  }

  .lex-morph-desc {
    margin: 0;
    font-size: 0.88rem;
  }

  .lex-occurrences {
    margin-top: 1rem;
    border-top: 1px solid var(--border, rgba(128, 128, 128, 0.25));
    padding-top: 0.75rem;
  }

  .occ-toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 0.35rem 0.25rem;
    background: transparent;
    border: 0;
    color: inherit;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
    text-align: left;
  }

  .occ-toggle-meta {
    font-size: 0.85em;
    opacity: 0.75;
  }

  .occ-body {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding-top: 0.5rem;
  }

  .occ-controls {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .occ-bible-label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.85rem;
  }

  .occ-bible-select {
    flex: 1;
    min-width: 0;
    padding: 0.25rem 0.4rem;
    border-radius: 4px;
    border: 1px solid var(--border, rgba(128, 128, 128, 0.35));
    background: var(--surface, transparent);
    color: inherit;
    font: inherit;
  }

  .occ-summary {
    margin: 0;
    font-size: 0.85rem;
    opacity: 0.8;
  }

  .occ-block {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .occ-subtitle {
    margin: 0;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.7;
  }

  .occ-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .occ-chip {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.15rem 0.5rem;
    border-radius: 999px;
    border: 1px solid var(--border, rgba(128, 128, 128, 0.35));
    font-size: 0.82rem;
  }

  .occ-books {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .occ-book-row {
    display: grid;
    grid-template-columns: 3.2rem 1fr 2.6rem;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.82rem;
  }

  .occ-book-name {
    font-weight: 500;
  }

  .occ-bar-track {
    height: 6px;
    border-radius: 3px;
    background: var(--border, rgba(128, 128, 128, 0.2));
    overflow: hidden;
  }

  .occ-bar-fill {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--accent, #c28b42);
  }

  .occ-book-count {
    text-align: right;
    font-variant-numeric: tabular-nums;
    opacity: 0.85;
  }

  .occ-refs {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .occ-ref-btn {
    padding: 0.2rem 0.45rem;
    border-radius: 4px;
    border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 0.82rem;
    cursor: pointer;
  }

  .occ-ref-btn:hover,
  .occ-ref-btn:focus-visible {
    border-color: var(--accent, #c28b42);
  }

  .occ-pagination {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    margin-top: 0.35rem;
  }

  .occ-page-info {
    font-size: 0.8rem;
    opacity: 0.8;
  }
</style>
