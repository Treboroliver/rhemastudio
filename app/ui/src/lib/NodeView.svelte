<script lang="ts">
  import NodeView from './NodeView.svelte'
  import FootnoteMarker from './FootnoteMarker.svelte'
  import { strongChain, type NodeActions, type UiNode } from './reader'

  interface Props {
    nodes: UiNode[]
    t: (key: string, values?: Record<string, string | number>) => string
    showStrong?: boolean
    actions?: NodeActions
  }

  let { nodes, t, showStrong = false, actions }: Props = $props()
</script>

{#each nodes as node}
  {#if node.type === 'text'}
    {node.value}
  {:else if node.type === 'italic'}
    <em class="node-italic"><NodeView nodes={node.children} {t} {showStrong} {actions} /></em>
  {:else if node.type === 'bold'}
    <strong class="node-bold"><NodeView nodes={node.children} {t} {showStrong} {actions} /></strong>
  {:else if node.type === 'underline'}
    <u><NodeView nodes={node.children} {t} {showStrong} {actions} /></u>
  {:else if node.type === 'strikethrough'}
    <s><NodeView nodes={node.children} {t} {showStrong} {actions} /></s>
  {:else if node.type === 'caps'}
    <span class="node-caps"><NodeView nodes={node.children} {t} {showStrong} {actions} /></span>
  {:else if node.type === 'superscript'}
    <sup><NodeView nodes={node.children} {t} {showStrong} {actions} /></sup>
  {:else if node.type === 'subscript'}
    <sub><NodeView nodes={node.children} {t} {showStrong} {actions} /></sub>
  {:else if node.type === 'wordsOfJesus'}
    <span class="words-of-jesus"><NodeView nodes={node.children} {t} {showStrong} {actions} /></span>
  {:else if node.type === 'otQuote'}
    <span class="ot-quote"><NodeView nodes={node.children} {t} {showStrong} {actions} /></span>
  {:else if node.type === 'link'}
    <span class="node-link unknown-link" title={node.hint ? t('unknownLinkHint', { type: node.hint }) : t('unknownLinkHintGeneric')}><NodeView nodes={node.children} {t} {showStrong} {actions} /></span>
  {:else if node.type === 'verseLink'}
    {#if actions?.onVerse}
      <button
        class="rich-link verse-link"
        type="button"
        data-verse-link={`${node.book}.${node.chapter}.${node.verse}`}
        onclick={() => actions.onVerse?.({ book: node.book, chapter: node.chapter, verse: node.verse })}
      ><NodeView nodes={node.children} {t} {showStrong} {actions} /></button>
    {:else}
      <span class="node-link"><NodeView nodes={node.children} {t} {showStrong} {actions} /></span>
    {/if}
  {:else if node.type === 'entryLink'}
    {#if actions?.onEntry}
      <button
        class="rich-link entry-link"
        type="button"
        data-entry-link={`${node.lang}${node.num}`}
        onclick={() => actions.onEntry?.(`${node.lang}${node.num}`)}
      ><NodeView nodes={node.children} {t} {showStrong} {actions} /></button>
    {:else}
      <span class="node-link"><NodeView nodes={node.children} {t} {showStrong} {actions} /></span>
    {/if}
  {:else if node.type === 'heading'}
    <span class="section-title" role="heading" aria-level={Math.min(node.level + 2, 6)}><NodeView nodes={node.children} {t} {showStrong} {actions} /></span>
  {:else if node.type === 'strong'}
    {@const chain = strongChain(node)}
    <button
      class="strong-word"
      class:show-strong={showStrong}
      data-strong={chain.ids.join(' ')}
      type="button"
      onclick={(event) => actions?.onStrong?.(chain.ids, event.currentTarget)}
    ><NodeView nodes={chain.children} {t} {showStrong} {actions} /></button>
  {:else if node.type === 'note'}
    <FootnoteMarker {t}><NodeView nodes={node.children} {t} {showStrong} {actions} /></FootnoteMarker>
  {:else if node.type === 'paragraph'}
    <span class="paragraph-break" aria-hidden="true"></span>
  {:else if node.type === 'lineBreak'}
    <br />
  {/if}
{/each}
