<script lang="ts">
  import { cn } from "$lib/utils";

  interface Props extends svelte.JSX.HTMLAttributes<HTMLButtonElement> {
    class?: string;
    children?: any;
  }

  let { class: className, children, ...rest }: Props = $props();
</script>

<button class={cn("inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 h-10 px-4 py-2", className)} {...rest}>
  {#if children}
    {@render children()}
  {/if}
</button>
