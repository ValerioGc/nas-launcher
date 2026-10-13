import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router';
import { useAuthStore } from '@/stores/authStore';
import { useSetupStore } from '@/stores/setupStore';
import { pinia } from '@/stores/pinia';

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    redirect: { name: 'login' },
  },
  {
    path: '/:pathMatch(.*)*',
    redirect: { name: 'login' },
  },
  {
    path: '/login',
    name: 'login',
    component: () => import('@/views/LoginPage.vue'),
  },
  {
    path: '/console',
    name: 'main',
    component: () => import('@/views/MainPage.vue'),
  },
  {
    path: '/preparing',
    name: 'preparing',
    component: () => import('@/views/PreparingView.vue'),
  },
];

export const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes,
});

/**
 * Navigation guard to handle routing based on the setup phase and authentication status.
 */
router.beforeEach(async (to) => {
  const phase = await useSetupStore(pinia).check();
  if (phase !== 'ready') {
    return to.name === 'preparing' || { name: 'preparing' };
  }

  if (to.name === 'preparing') {
    return { name: 'login' };
  }

  if (to.name !== 'main') {
    return true;
  }

  return (await useAuthStore(pinia).checkSession()) || { name: 'login' };
});

export default router;
