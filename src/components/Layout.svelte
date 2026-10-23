<script lang="ts">
  import { cn } from "$lib/utils";
</script>

<div class={cn("flex flex-col min-h-screen", $$props.class)}>
  <slot />
</div>
