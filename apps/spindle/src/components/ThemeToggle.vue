<script setup lang="ts">
import { useThemeStore } from '@/stores/themeStore';
import { computed } from 'vue';

const themeStore = useThemeStore()
const isDark = computed(() => {
    return themeStore.theme === "dark"
})

async function toggle() {
    await themeStore.toggleTheme()
}
</script>

<template>
    <button class="theme-toggle" :class="{ 'theme-toggle--dark': isDark }" title="Toggles light & dark"
        aria-label="auto" aria-live="polite" @click="toggle">
        <svg class="sun-and-moon" aria-hidden="true" width="24" height="24" viewBox="0 0 24 24">
            <mask class="moon" id="moon-mask">
                <rect x="0" y="0" width="100%" height="100%" fill="white" />
                <circle cx="24" cy="10" r="6" fill="black" />
            </mask>
            <circle class="sun" cx="12" cy="12" r="6" mask="url(#moon-mask)" fill="currentColor" />
            <g class="sun-beams" stroke="currentColor">
                <line x1="12" y1="1" x2="12" y2="3" />
                <line x1="12" y1="21" x2="12" y2="23" />
                <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
                <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
                <line x1="1" y1="12" x2="3" y2="12" />
                <line x1="21" y1="12" x2="23" y2="12" />
                <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
                <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
            </g>
        </svg>
    </button>
</template>



<style lang="css" scoped>
.theme-toggle {
    inline-size: 100%;
    block-size: 100%;
    --icon-fill: currentColor;

    background: none;
    border: none;
    padding: 0;
    aspect-ratio: 1;
    border-radius: 50%;
    cursor: pointer;
    touch-action: manipulation;
    -webkit-tap-highlight-color: transparent;
    outline-offset: 5px;
    color: inherit;
}

.theme-toggle>svg {
    inline-size: 100%;
    block-size: 100%;
    stroke-linecap: round;
}

.sun-beams {
    stroke: var(--icon-fill);
    stroke-width: 2px;
    transform-origin: center center;
    transition: transform .5s cubic-bezier(.5, 1.25, .75, 1.25), opacity .5s cubic-bezier(.25, 0, .3, 1);
}

.sun {
    fill: var(--icon-fill);
    transform-origin: center center;
    transition: transform .5s cubic-bezier(.25, 0, .3, 1);
}

.moon>circle {
    transition: transform .25s cubic-bezier(0, 0, 0, 1);
}

.theme-toggle--dark .sun-beams {
    transform: rotate(-25deg);
    opacity: 0;
    transition-duration: .15s;
}

.theme-toggle--dark .sun {
    transform: scale(1.75);
    transition-timing-function: cubic-bezier(.25, 0, .3, 1);
    transition-duration: .25s;
}

.theme-toggle--dark .moon>circle {
    transform: translateX(-7px);
    transition-delay: .25s;
    transition-duration: .5s;
}

@media (hover: none) {
    .theme-toggle {
        inline-size: 48px;
        block-size: 48px;
    }
}
</style>