import { createRouter, createWebHistory } from 'vue-router'
import ServicesView from '@/views/ServicesView.vue'

const routes = [
    {path: '/', redirect: '/services'},
    {
        name: 'Services',
        path: '/services',
        component: ServicesView,
    }
]

const router = createRouter({
    history: createWebHistory(),
    routes
})

export default router