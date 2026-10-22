<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  
  interface PetConfig {
    name: string;
    spritesheet: {
      width: number;
      height: number;
      frameWidth: number;
      frameHeight: number;
      columns: number;
      rows: number;
    };
    states: Record<string, { row: number; frames: number; fps: number }>;
    mapping: Record<string, string>;
  }

  interface AppState {
    status: 'idle' | 'working' | 'paused' | 'interrupt';
  }

  let config: PetConfig | null = null;
  let currentState = $state('idle');
  let currentFrame = $state(0);
  let animationInterval: ReturnType<typeof setInterval> | null = null;
  let stateCheckInterval: ReturnType<typeof setInterval> | null = null;

  async function loadPetConfig() {
    // For default pet, use embedded config
    config = {
      name: "番茄君",
      spritesheet: { width: 1536, height: 1872, frameWidth: 192, frameHeight: 208, columns: 8, rows: 9 },
      states: {
        idle: { row: 0, frames: 8, fps: 8 },
        wave: { row: 1, frames: 8, fps: 8 },
        jump: { row: 2, frames: 8, fps: 10 },
        failed: { row: 3, frames: 8, fps: 6 },
        waiting: { row: 4, frames: 8, fps: 4 },
        review: { row: 5, frames: 8, fps: 6 }
      },
      mapping: {
        working: "idle",
        interrupt: "failed",
        unrecorded: "wave",
        done: "jump",
        lunch: "waiting",
        paused: "review"
      }
    };
  }

  async function checkAppState() {
    try {
      const state: AppState = await invoke('get_app_state');
      const mappedState = config?.mapping[state.status] || 'idle';
      if (mappedState !== currentState) {
        currentState = mappedState;
        currentFrame = 0;
      }
    } catch (err) {
      console.error('Failed to get app state:', err);
    }
  }

  function startAnimation() {
    if (!config) return;
    
    const stateConfig = config.states[currentState];
    if (!stateConfig) return;

    if (animationInterval) clearInterval(animationInterval);
    
    animationInterval = setInterval(() => {
      currentFrame = (currentFrame + 1) % stateConfig.frames;
    }, 1000 / stateConfig.fps);
  }

  $effect(() => {
    startAnimation();
  });

  onMount(async () => {
    await loadPetConfig();
    await checkAppState();
    stateCheckInterval = setInterval(checkAppState, 2000);
  });

  onDestroy(() => {
    if (animationInterval) clearInterval(animationInterval);
    if (stateCheckInterval) clearInterval(stateCheckInterval);
  });

  const spritePosition = $derived(() => {
    if (!config) return { x: 0, y: 0 };
    const stateConfig = config.states[currentState];
    if (!stateConfig) return { x: 0, y: 0 };
    
    const x = -(currentFrame * config.spritesheet.frameWidth);
    const y = -(stateConfig.row * config.spritesheet.frameHeight);
    return { x, y };
  });
</script>

<div class="relative w-48 h-52">
  {#if config}
    <div 
      class="sprite"
      style="
        width: {config.spritesheet.frameWidth}px;
        height: {config.spritesheet.frameHeight}px;
        background-image: url('/pets/default/spritesheet.png');
        background-position: {spritePosition().x}px {spritePosition().y}px;
        image-rendering: pixelated;
      "
    />
  {/if}
</div>

<style>
  .sprite {
    background-repeat: no-repeat;
  }
</style>
