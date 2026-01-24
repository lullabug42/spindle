<script setup lang="ts">
import { ref } from 'vue';
import ToggleButton from '@/components/services-view/ToggleButton.vue';
import { NFlex } from 'naive-ui';

const viewMode = ref<'grid' | 'graph'>('grid');

</script>

<template>
    <n-flex :wrap="false" align="center">
        <ToggleButton v-model:view-mode="viewMode" icon-size="1.5rem" />

    </n-flex>
</template>

<style lang="css" scoped></style>