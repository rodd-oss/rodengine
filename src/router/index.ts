import { createRouter, createWebHistory } from 'vue-router'
import Layout from '../layouts/Layout.vue'
import SchemaEditor from '../views/SchemaEditor.vue'
import DataViewer from '../views/DataViewer.vue'
import QueryBuilder from '../views/QueryBuilder.vue'
import ReplicationDashboard from '../views/ReplicationDashboard.vue'
import PerformanceProfiling from '../views/PerformanceProfiling.vue'

const routes = [
  {
    path: '/',
    component: Layout,
    children: [
      {
        path: '',
        redirect: '/schema'
      },
      {
        path: 'schema',
        name: 'SchemaEditor',
        component: SchemaEditor
      },
      {
        path: 'data',
        name: 'DataViewer',
        component: DataViewer
      },
      {
        path: 'query',
        name: 'QueryBuilder',
        component: QueryBuilder
      },
      {
        path: 'replication',
        name: 'ReplicationDashboard',
        component: ReplicationDashboard
      },
      {
        path: 'performance',
        name: 'PerformanceProfiling',
        component: PerformanceProfiling
      }
    ]
  }
]

const router = createRouter({
  history: createWebHistory(),
  routes
})

export default router