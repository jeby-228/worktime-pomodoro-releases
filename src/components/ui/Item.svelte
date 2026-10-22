<script lang="ts">
  import { cn } from "$lib/utils";

  interface Props extends svelte.JSX.HTMLAttributes<HTMLDivElement> {
    class?: string;
    type?: "default" | "outline" | "muted" | "dashed";
    size?: "sm" | "md";
    children?: any;
  }

  let { class: className, type = "default", size = "md", children, ...rest }: Props = $props();

  const variantClasses = {
    default: "bg-primary text-primary-foreground border border-border shadow-sm",
    outline: "bg-card border border-border",
    muted: "bg-muted border border-border",
    dashed: "bg-card border border-dashed border-border",
  };

  const sizeClasses = {
    sm: "min-h-[36px] text-sm",
    md: "min-h-[44px]",
  };
</script>

<div class={cn("flex items-center gap-3 px-3 py-2 rounded-lg", variantClasses[type], sizeClasses[size], className)} {...rest}>
  {#if children}
    {@render children()}
  {/if}
</div>
