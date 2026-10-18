<script lang="ts">
  let activeTab = $state<'estudo' | 'pulpito' | 'projecao'>('estudo');
  let currentRef = $state('João 3:16');
  let currentTranslation = $state('ARA');
  let selectedVerse = $state(16);
</script>

<div class="flex flex-col h-screen w-screen bg-[#0f1115] text-[#e2e8f0] select-none overflow-hidden">
  <!-- Top Bar / Barra Superior do Protótipo -->
  <header class="h-14 bg-[#161920] border-b border-[#2a2f3d] px-4 flex items-center justify-between gap-4 shrink-0">
    <!-- Brand / Logo -->
    <div class="flex items-center gap-3">
      <div class="w-8 h-8 rounded-lg bg-gradient-to-br from-[#d97706] to-[#b45309] flex items-center justify-center shadow-lg shadow-amber-900/30">
        <svg class="w-5 h-5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
        </svg>
      </div>
      <div>
        <h1 class="text-base font-bold tracking-tight text-white flex items-center gap-2">
          RhemaStudio
          <span class="text-[10px] uppercase tracking-wider font-semibold px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20">M0</span>
        </h1>
        <p class="text-[11px] text-gray-400">Estudo bíblico e preparação de sermões</p>
      </div>
    </div>

    <!-- Center Command Bar (Ctrl+K) -->
    <div class="flex-1 max-w-xl">
      <button
        class="w-full h-9 bg-[#1e222d] hover:bg-[#252b39] border border-[#2a2f3d] rounded-lg px-3 flex items-center justify-between text-sm text-gray-400 transition-colors shadow-inner group"
      >
        <div class="flex items-center gap-2">
          <svg class="w-4 h-4 text-gray-400 group-hover:text-amber-400 transition-colors" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          <span class="text-xs text-gray-300">Ir para passagem, buscar Strong ou digitar comando...</span>
        </div>
        <kbd class="px-2 py-0.5 text-[10px] font-semibold bg-[#161920] border border-[#2a2f3d] rounded text-gray-400 group-hover:text-amber-300 group-hover:border-amber-500/30">Ctrl+K</kbd>
      </button>
    </div>

    <!-- Mode Selector & Controls -->
    <div class="flex items-center gap-2">
      <div class="flex bg-[#1e222d] p-1 rounded-lg border border-[#2a2f3d]">
        <button
          onclick={() => activeTab = 'estudo'}
          class="px-3 py-1 text-xs font-medium rounded-md transition-all {activeTab === 'estudo' ? 'bg-[#d97706] text-white shadow' : 'text-gray-400 hover:text-white'}"
        >
          Estudo
        </button>
        <button
          onclick={() => activeTab = 'pulpito'}
          class="px-3 py-1 text-xs font-medium rounded-md transition-all {activeTab === 'pulpito' ? 'bg-[#d97706] text-white shadow' : 'text-gray-400 hover:text-white'}"
        >
          Púlpito (Ctrl+P)
        </button>
        <button
          onclick={() => activeTab = 'projecao'}
          class="px-3 py-1 text-xs font-medium rounded-md transition-all {activeTab === 'projecao' ? 'bg-[#d97706] text-white shadow' : 'text-gray-400 hover:text-white'}"
        >
          Projeção
        </button>
      </div>

      <div class="h-6 w-[1px] bg-[#2a2f3d] mx-1"></div>

      <!-- Quick settings button -->
      <button
        title="Configurações"
        class="w-8 h-8 rounded-lg bg-[#1e222d] hover:bg-[#252b39] border border-[#2a2f3d] flex items-center justify-center text-gray-400 hover:text-white transition-colors"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
        </svg>
      </button>
    </div>
  </header>

  <!-- Main Content Layout (Estudo: 3 Panels) -->
  <main class="flex-1 flex overflow-hidden">
    <!-- Left Sidebar: Biblioteca de Módulos -->
    <aside class="w-64 bg-[#161920] border-r border-[#2a2f3d] flex flex-col shrink-0">
      <div class="p-3 border-b border-[#2a2f3d] flex items-center justify-between">
        <span class="text-xs font-semibold uppercase tracking-wider text-gray-400">Biblioteca</span>
        <span class="text-[11px] text-amber-400 font-mono">theWord</span>
      </div>
      <div class="p-3 flex-1 overflow-y-auto space-y-3">
        <div>
          <span class="text-[11px] font-semibold text-gray-400 uppercase tracking-wider block mb-1.5">Bíblias (.ont / .nt / .ot)</span>
          <div class="space-y-1">
            <button class="w-full text-left px-2.5 py-1.5 rounded bg-amber-500/10 text-amber-300 border border-amber-500/20 text-xs font-medium flex items-center justify-between">
              <span>Almeida Revista e Atualizada</span>
              <span class="text-[10px] bg-amber-500/20 px-1 rounded">ARA</span>
            </button>
            <button class="w-full text-left px-2.5 py-1.5 rounded hover:bg-[#1e222d] text-gray-300 text-xs flex items-center justify-between transition-colors">
              <span>Almeida Corrigida Fiel</span>
              <span class="text-[10px] text-gray-500">ACF</span>
            </button>
            <button class="w-full text-left px-2.5 py-1.5 rounded hover:bg-[#1e222d] text-gray-300 text-xs flex items-center justify-between transition-colors">
              <span>King James Version</span>
              <span class="text-[10px] text-gray-500">KJV</span>
            </button>
          </div>
        </div>

        <div>
          <span class="text-[11px] font-semibold text-gray-400 uppercase tracking-wider block mb-1.5">Comentários (.cmt.twm)</span>
          <div class="space-y-1">
            <div class="px-2.5 py-1.5 rounded bg-[#1e222d]/60 text-gray-400 text-xs">
              Comentário Bíblico Moody
            </div>
          </div>
        </div>

        <div>
          <span class="text-[11px] font-semibold text-gray-400 uppercase tracking-wider block mb-1.5">Dicionários & Léxico (.dct.twm)</span>
          <div class="space-y-1">
            <div class="px-2.5 py-1.5 rounded bg-[#1e222d]/60 text-gray-400 text-xs flex items-center justify-between">
              <span>Léxico de Strong (Grego/Hebraico)</span>
              <span class="text-[10px] text-amber-400">Ativo</span>
            </div>
          </div>
        </div>
      </div>
    </aside>

    <!-- Center Bible Pane (Texto Bíblico) -->
    <section class="flex-1 flex flex-col bg-[#0f1115] overflow-hidden">
      <!-- Chapter Header -->
      <div class="h-10 border-b border-[#2a2f3d] px-6 flex items-center justify-between bg-[#13161c]">
        <div class="flex items-center gap-3">
          <button class="p-1 hover:bg-[#1e222d] rounded text-gray-400 hover:text-white transition-colors" title="Capítulo Anterior (Ctrl+←)">
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <span class="text-sm font-semibold text-white">{currentRef}</span>
          <span class="text-xs text-gray-400">({currentTranslation})</span>
          <button class="p-1 hover:bg-[#1e222d] rounded text-gray-400 hover:text-white transition-colors" title="Capítulo Seguinte (Ctrl+→)">
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
            </svg>
          </button>
        </div>

        <div class="flex items-center gap-2 text-xs text-gray-400">
          <span class="px-2 py-0.5 rounded bg-[#1e222d] border border-[#2a2f3d]">Novo Testamento</span>
          <span class="px-2 py-0.5 rounded bg-[#1e222d] border border-[#2a2f3d]">Grego Textus Receptus</span>
        </div>
      </div>

      <!-- Verses Display (Tipografia Serifada de Leitura) -->
      <div class="flex-1 p-8 overflow-y-auto max-w-3xl mx-auto w-full">
        <!-- Section title -->
        <h2 class="text-sm uppercase tracking-widest font-semibold text-amber-400/90 mb-6 text-center border-b border-amber-500/20 pb-2">
          Deus amou o mundo
        </h2>

        <div class="space-y-4 font-serif text-lg leading-relaxed text-[#f1f5f9]">
          <div
            role="button"
            tabindex="0"
            class="p-3 rounded-lg transition-colors cursor-pointer text-left w-full {selectedVerse === 14 ? 'bg-[#1e222d] ring-1 ring-amber-500/30' : 'hover:bg-[#161920]'}"
            onclick={() => selectedVerse = 14}
            onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') selectedVerse = 14; }}
          >
            <span class="font-sans text-xs font-bold text-amber-500 mr-2 select-none">14</span>
            E do modo por que Moisés levantou a serpente no deserto, assim importa que o Filho do Homem seja levantado,
          </div>

          <div
            role="button"
            tabindex="0"
            class="p-3 rounded-lg transition-colors cursor-pointer text-left w-full {selectedVerse === 15 ? 'bg-[#1e222d] ring-1 ring-amber-500/30' : 'hover:bg-[#161920]'}"
            onclick={() => selectedVerse = 15}
            onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') selectedVerse = 15; }}
          >
            <span class="font-sans text-xs font-bold text-amber-500 mr-2 select-none">15</span>
            para que todo o que nele crê tenha a vida eterna.
          </div>

          <div
            role="button"
            tabindex="0"
            class="p-4 rounded-xl transition-all cursor-pointer text-left w-full bg-[#1e222d]/80 ring-2 ring-amber-500/50 shadow-lg"
            onclick={() => selectedVerse = 16}
            onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') selectedVerse = 16; }}
          >
            <span class="font-sans text-xs font-bold text-amber-400 mr-2 select-none">16</span>
            Porque Deus
            <span class="underline decoration-dotted decoration-amber-400 hover:bg-amber-400/20 px-1 py-0.5 rounded transition-colors text-amber-300 font-sans text-base">
              amou
              <sup class="text-[10px] text-amber-400 ml-0.5">G25</sup>
            </span>
            ao mundo de tal maneira que deu o seu Filho unigênito, para que todo o que nele crê não pereça, mas tenha a
            <span class="underline decoration-dotted decoration-amber-400 hover:bg-amber-400/20 px-1 py-0.5 rounded transition-colors text-amber-300 font-sans text-base">
              vida eterna
              <sup class="text-[10px] text-amber-400 ml-0.5">G2222</sup>
            </span>.
            <span class="inline-block ml-2 select-none">
              <span class="text-xs px-2 py-0.5 rounded-full bg-blue-500/20 text-[#60a5fa] border border-blue-500/30 font-sans">
                Rm 5:8; 1Jo 4:9
              </span>
            </span>
          </div>

          <div
            role="button"
            tabindex="0"
            class="p-3 rounded-lg transition-colors cursor-pointer text-left w-full {selectedVerse === 17 ? 'bg-[#1e222d] ring-1 ring-amber-500/30' : 'hover:bg-[#161920]'}"
            onclick={() => selectedVerse = 17}
            onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') selectedVerse = 17; }}
          >
            <span class="font-sans text-xs font-bold text-amber-500 mr-2 select-none">17</span>
            Porquanto Deus enviou o seu Filho ao mundo, não para que julgasse o mundo, mas para que o mundo fosse salvo por ele.
          </div>
        </div>
      </div>
    </section>

    <!-- Right Side Panel: Léxico / Comentário / Sermão -->
    <aside class="w-80 bg-[#161920] border-l border-[#2a2f3d] flex flex-col shrink-0">
      <!-- Tabs -->
      <div class="flex border-b border-[#2a2f3d] bg-[#13161c]">
        <button class="flex-1 py-2 text-xs font-medium text-amber-400 border-b-2 border-amber-500">
          Léxico Strong
        </button>
        <button class="flex-1 py-2 text-xs font-medium text-gray-400 hover:text-white">
          Comentários
        </button>
        <button class="flex-1 py-2 text-xs font-medium text-gray-400 hover:text-white">
          Sermão
        </button>
      </div>

      <!-- Lexicon Tab Content -->
      <div class="p-4 flex-1 overflow-y-auto space-y-4">
        <div class="p-3 rounded-lg bg-[#1e222d] border border-amber-500/30">
          <div class="flex items-center justify-between mb-1">
            <span class="font-mono text-sm font-bold text-amber-400">G25 — agapaō (ἀγαπάω)</span>
            <span class="text-[10px] bg-amber-500/20 text-amber-300 px-1.5 py-0.5 rounded">Verbo</span>
          </div>
          <p class="text-xs text-gray-300 leading-relaxed mt-2">
            Amar com benevolência profunda, devoção abnegada e consideração ativa; o amor divino demonstrado na cruz.
          </p>
          <div class="mt-3 pt-2 border-t border-[#2a2f3d] flex items-center justify-between text-[11px] text-gray-400">
            <span>Ocorrências no NT: 142x</span>
            <button class="text-amber-400 hover:underline">Ver todas</button>
          </div>
        </div>

        <div class="p-3 rounded-lg bg-[#1e222d] border border-[#2a2f3d]">
          <h3 class="text-xs font-semibold text-gray-300 mb-2">Preparação de Sermão</h3>
          <p class="text-xs text-gray-400 mb-3">
            Pressione <kbd class="px-1 py-0.5 bg-[#161920] border border-[#2a2f3d] rounded text-amber-400">Ctrl+Enter</kbd> para inserir o versículo selecionado no sermão.
          </p>
          <div class="p-2 rounded bg-[#161920] border border-[#2a2f3d] text-xs font-mono text-gray-300">
            /sermão "O Amor Incondicional"
          </div>
        </div>
      </div>
    </aside>
  </main>

  <!-- Bottom Status Bar -->
  <footer class="h-7 bg-[#161920] border-t border-[#2a2f3d] px-4 flex items-center justify-between text-[11px] text-gray-400 shrink-0">
    <div class="flex items-center gap-4">
      <span class="flex items-center gap-1.5 text-emerald-400">
        <span class="w-2 h-2 rounded-full bg-emerald-500 animate-pulse"></span>
        Cânon protestante: 66 livros (31.102 versículos)
      </span>
      <span>•</span>
      <span>Offline-first</span>
      <span>•</span>
      <span>Autosave: ativo (500 ms)</span>
    </div>

    <div class="flex items-center gap-3">
      <span class="text-gray-400"><kbd class="bg-[#1e222d] px-1 py-0.5 rounded border border-[#2a2f3d]">Ctrl+K</kbd> Comandos</span>
      <span class="text-gray-400"><kbd class="bg-[#1e222d] px-1 py-0.5 rounded border border-[#2a2f3d]">Ctrl+P</kbd> Púlpito</span>
      <span class="text-gray-400"><kbd class="bg-[#1e222d] px-1 py-0.5 rounded border border-[#2a2f3d]">Ctrl+Shift+P</kbd> Projeção</span>
    </div>
  </footer>
</div>
