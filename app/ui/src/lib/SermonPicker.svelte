<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { asSermonError, sermonCreate, sermonList, type SermonSummaryDto } from './sermon'

  interface Props {
    t: (key: string, values?: Record<string, string | number>) => string
    onPick: (id: string) => void
    onCancel: () => void
  }

  let { t, onPick, onCancel }: Props = $props()

  let items = $state<SermonSummaryDto[] | null>(null)
  let error = $state<string | null>(null)
  let busy = $state(false)
  let root = $state<HTMLElement | null>(null)

  onMount(async () => {
    try {
      items = (await sermonList(false, '')).slice(0, 5)
    } catch (cause) {
      items = []
      error = asSermonError(cause).code === 'sermon_unavailable' ? t('sermonUnavailable') : t('sermonLoadError')
    }
    await tick()
    focusables()[0]?.focus()
  })

  const focusables = () => Array.from(root?.querySelectorAll<HTMLElement>('button:not([disabled])') ?? [])

  async function createNew() {
    if (busy) return
    busy = true
    try {
      const created = await sermonCreate(t('sermonUntitled'))
      onPick(created.id)
    } catch {
      error = t('sermonCreateError')
      busy = false
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      onCancel()
    } else if (event.key === 'Tab') {
      const list = focusables()
      if (!list.length) return
      const first = list[0]
      const last = list[list.length - 1]
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }
  }
</script>

<div class="command-backdrop" role="presentation" onclick={onCancel}></div>
<!-- O Esc e o Tab são tratados no diálogo; o foco já está dentro dele. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="sermon-picker" role="dialog" tabindex="-1" aria-modal="true" aria-label={t('sermonPickerTitle')} bind:this={root} onkeydown={onKeydown}>
  <h2>{t('sermonPickerTitle')}</h2>
  {#if items === null}
    <p class="panel-note" role="status">{t('sermonLoading')}</p>
  {:else}
    {#if error}<p class="sermon-banner" role="alert">{error}</p>{/if}
    <ul>
      {#each items as item (item.id)}
        <li><button type="button" class="sermon-picker-item" onclick={() => onPick(item.id)}>{item.title}</button></li>
      {/each}
    </ul>
    <button type="button" class="primary-action" disabled={busy} onclick={() => void createNew()}>{t('sermonNew')}</button>
  {/if}
</div>
