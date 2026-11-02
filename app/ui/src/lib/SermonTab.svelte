<script lang="ts">
  import { onMount } from 'svelte'
  import SermonEditor from './SermonEditor.svelte'
  import SermonList from './SermonList.svelte'
  import { CLOSE_FLUSH_TIMEOUT_MS } from './sermon'
  import type { SermonBridge } from './sermonSession'

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string
    language: string
    books: { number: number; abbreviation: string }[]
    /** A aba está visível; ao deixar de estar, grava o que estiver pendente. */
    active: boolean
    bridge?: SermonBridge
  }

  let { t, language, books, active, bridge }: Props = $props()

  let openId = $state<string | null>(null)
  let editor = $state<ReturnType<typeof SermonEditor> | null>(null)

  export function flush(): Promise<boolean> {
    return editor ? editor.flush() : Promise.resolve(true)
  }

  $effect(() => {
    if (!active) void editor?.flush()
  })

  // Fechar a janela espera a gravação por no máximo 1 s; não impede o fechamento.
  onMount(() => {
    let unlisten: (() => void) | undefined
    let disposed = false
    void (async () => {
      try {
        if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) return
        const { getCurrentWindow } = await import('@tauri-apps/api/window')
        const stop = await getCurrentWindow().onCloseRequested(async () => {
          await Promise.race([
            flush().catch(() => false),
            new Promise((resolve) => setTimeout(resolve, CLOSE_FLUSH_TIMEOUT_MS))
          ])
        })
        if (disposed) stop()
        else unlisten = stop
      } catch {
        // Fora do Tauri (testes, navegador) não há janela para observar.
      }
    })()
    return () => {
      disposed = true
      unlisten?.()
    }
  })
</script>

{#if openId}
  {#key openId}
    <SermonEditor bind:this={editor} {t} {language} {books} {bridge} id={openId} onBack={() => ((openId = null), (editor = null))} />
  {/key}
{:else}
  <SermonList {t} {language} onOpen={(id) => (openId = id)} />
{/if}
