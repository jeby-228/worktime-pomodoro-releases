<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  interface StateData {
    status: 'idle' | 'working' | 'interrupt' | 'paused';
    current_slot_id: number | null;
    paused_slot_id: number | null;
    remaining_seconds: number | null;
    pomodoro_start: string | null;
  }

  let currentState = $state<string>('idle');
  let stateCheckInterval: ReturnType<typeof setInterval> | null = null;

  const stateMapping: Record<string, string> = {
    idle: 'lunch',
    working: 'working',
    interrupt: 'interrupt',
    paused: 'done'
  };

  async function checkAppState() {
    try {
      const state: StateData = await invoke('get_app_state');
      const mappedState = stateMapping[state.status] || 'lunch';
      if (mappedState !== currentState) {
        currentState = mappedState;
      }
    } catch (err) {
      console.error('Failed to get app state:', err);
    }
  }

  onMount(async () => {
    await checkAppState();
    stateCheckInterval = setInterval(checkAppState, 2000);
  });

  onDestroy(() => {
    if (stateCheckInterval) clearInterval(stateCheckInterval);
  });

  const svgPath = $derived(`/pets/dmono/${currentState}.svg`);
</script>

<div class="pet-container">
  <img src={svgPath} alt="番茄君" class="pet-image" />
</div>

<style>
  .pet-container {
    width: 192px;
    height: 208px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .pet-image {
    width: 96px;
    height: 96px;
    filter: drop-shadow(0 2px 3px rgba(0, 0, 0, 0.25));
  }
</style>
