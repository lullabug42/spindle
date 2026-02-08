import { createRouter, createWebHistory } from "vue-router";
import ServicesOverview from "@/views/services-view/ServicesOverview.vue";
import ServiceGroupDetail from "@/views/services-view/ServiceGroupDetail.vue";

const routes = [
  { path: "/", redirect: "/services" },
  {
    name: "Services",
    path: "/services",
    component: ServicesOverview,
  },
  {
    name: "ServiceGroupDetail",
    path: "/services/group/:groupId",
    component: ServiceGroupDetail,
  },
];

const router = createRouter({
  history: createWebHistory(),
  routes,
});

export default router;