<script lang="ts">
  import NodeView from './NodeView.svelte'
  import FootnoteMarker from './FootnoteMarker.svelte'
  import type { UiNode } from './reader'

  interface Props {
    nodes: UiNode[]
    t: (key: string) => string
    showStrong?: boolean
  }

  let { nodes, t, showStrong = false }: Props = $props()
</script>

{#each nodes as node}
  {#if node.type === 'text'}
    {node.value}
  {:else if node.type === 'italic'}
    <em class="node-italic"><NodeView nodes={node.children} {t} {showStrong} /></em>
  {:else if node.type === 'bold'}
    <strong class="node-bold"><NodeView nodes={node.children} {t} {showStrong} /></strong>
  {:else if node.type === 'underline'}
    <u><NodeView nodes={node.children} {t} {showStrong} /></u>
  {:else if node.type === 'strikethrough'}
    <s><NodeView nodes={node.children} {t} {showStrong} /></s>
  {:else if node.type === 'caps'}
    <span class="node-caps"><NodeView nodes={node.children} {t} {showStrong} /></span>
  {:else if node.type === 'superscript'}
    <sup><NodeView nodes={node.children} {t} {showStrong} /></sup>
  {:else if node.type === 'subscript'}
    <sub><NodeView nodes={node.children} {t} {showStrong} /></sub>
  {:else if node.type === 'wordsOfJesus'}
    <span class="words-of-jesus"><NodeView nodes={node.children} {t} {showStrong} /></span>
  {:else if node.type === 'otQuote'}
    <span class="ot-quote"><NodeView nodes={node.children} {t} {showStrong} /></span>
  {:else if node.type === 'link'}
    <span class="node-link"><NodeView nodes={node.children} {t} {showStrong} /></span>
  {:else if node.type === 'heading'}
    <span class="section-title" role="heading" aria-level={Math.min(node.level + 2, 6)}><NodeView nodes={node.children} {t} {showStrong} /></span>
  {:else if node.type === 'strong'}
    <button class="strong-word" class:show-strong={showStrong} data-strong={`${node.lang}${node.num}`} type="button"><NodeView nodes={node.children} {t} {showStrong} /></button>
  {:else if node.type === 'note'}
    <FootnoteMarker {t}><NodeView nodes={node.children} {t} {showStrong} /></FootnoteMarker>
  {:else if node.type === 'paragraph'}
    <span class="paragraph-break" aria-hidden="true"></span>
  {:else if node.type === 'lineBreak'}
    <br />
  {/if}
{/each}
