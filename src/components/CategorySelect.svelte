<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { t } from '$lib/i18n';
  import type { Category } from '$lib/types';

  interface Props {
    value?: number | null;
    id?: string;
  }

  let { value = $bindable(null), id }: Props = $props();

  let categories = $state<Category[]>([]);

  // A hidden category stays listed on a record that already uses it.
  const options = $derived(categories.filter((category) => !category.hidden || category.id === value));

  onMount(async () => {
    try {
      categories = await invoke<Category[]>('get_categories');
    } catch (err) {
      console.error('Failed to load categories:', err);
    }
  });
</script>

<select
  {id}
  bind:value
  aria-label={id ? undefined : $t('categories.label')}
  class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-ring"
>
  <option value={null}>{$t('categories.uncategorized')}</option>
  {#each options as category (category.id)}
    <option value={category.id}>{category.name}</option>
  {/each}
</select>
