<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useAppStore } from '../store'

const appStore = useAppStore()
const serverStatus = ref('stopped')
const clientCount = ref(0)
const deltaQueueSize = ref(0)
const conflicts = ref<any[]>([])
const syncProgress = ref(0)

const startServer = () => {
  serverStatus.value = 'running'
  // TODO: call Rust command to start replication server
}

const stopServer = () => {
  serverStatus.value = 'stopped'
  // TODO: call Rust command to stop replication server
}

const addMockClient = () => {
  const clientId = appStore.connectedClients.length + 1
  const client = {
    id: `client-${clientId}`,
    address: `192.168.1.${clientId}:${8000 + clientId}`,
    version: '1.0.0',
    lastHeartbeat: new Date().toISOString(),
    lag: Math.floor(Math.random() * 100),
    status: 'connected'
  }
  appStore.addConnectedClient(client)
}

const addMockDelta = () => {
  const delta = {
    id: Date.now(),
    type: ['insert', 'update', 'delete'][Math.floor(Math.random() * 3)],
    table: ['entities', 'transform', 'health'][Math.floor(Math.random() * 3)],
    entityId: Math.floor(Math.random() * 1000),
    timestamp: new Date().toISOString(),
    size: Math.floor(Math.random() * 1000)
  }
  appStore.addDelta(delta)
}

const addMockConflict = () => {
  const conflict = {
    id: Date.now(),
    type: 'write-write',
    table: 'transform',
    entityId: Math.floor(Math.random() * 1000),
    resolution: 'server-wins',
    timestamp: new Date().toISOString()
  }
  conflicts.value.unshift(conflict)
  if (conflicts.value.length > 50) {
    conflicts.value.pop()
  }
}

let interval: number

onMounted(() => {
  interval = setInterval(() => {
    if (serverStatus.value === 'running') {
      deltaQueueSize.value = Math.floor(Math.random() * 100)
      clientCount.value = appStore.connectedClients.length
    }
  }, 1000)
})

onUnmounted(() => {
  clearInterval(interval)
})
</script>

<template>
  <div class="replication-dashboard">
    <div class="dashboard-header">
      <h2>Replication Dashboard</h2>
      <div class="server-controls">
        <button 
          class="btn" 
          :class="{ 'btn-success': serverStatus === 'running', 'btn-danger': serverStatus === 'stopped' }"
          @click="serverStatus === 'stopped' ? startServer() : stopServer()"
        >
          {{ serverStatus === 'stopped' ? 'Start Server' : 'Stop Server' }}
        </button>
        <button class="btn" @click="addMockClient">
          Add Test Client
        </button>
        <button class="btn" @click="addMockDelta">
          Simulate Delta
        </button>
        <button class="btn" @click="addMockConflict">
          Simulate Conflict
        </button>
      </div>
    </div>
    
    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-icon">🔄</div>
        <div class="stat-content">
          <div class="stat-value">{{ serverStatus.toUpperCase() }}</div>
          <div class="stat-label">Server Status</div>
        </div>
      </div>
      
      <div class="stat-card">
        <div class="stat-icon">👥</div>
        <div class="stat-content">
          <div class="stat-value">{{ appStore.connectedClients.length }}</div>
          <div class="stat-label">Connected Clients</div>
        </div>
      </div>
      
      <div class="stat-card">
        <div class="stat-icon">📨</div>
        <div class="stat-content">
          <div class="stat-value">{{ deltaQueueSize }}</div>
          <div class="stat-label">Delta Queue Size</div>
        </div>
      </div>
      
      <div class="stat-card">
        <div class="stat-icon">⚡</div>
        <div class="stat-content">
          <div class="stat-value">{{ syncProgress }}%</div>
          <div class="stat-label">Sync Progress</div>
        </div>
      </div>
    </div>
    
    <div class="dashboard-content">
      <div class="clients-panel">
        <h3>Connected Clients</h3>
        <div v-if="appStore.connectedClients.length === 0" class="empty-state">
          No clients connected
        </div>
        <div v-else class="clients-list">
          <div v-for="client in appStore.connectedClients" :key="client.id" class="client-card">
            <div class="client-header">
              <span class="client-id">{{ client.id }}</span>
              <span class="client-status" :class="client.status">{{ client.status }}</span>
            </div>
            <div class="client-details">
              <div class="client-address">{{ client.address }}</div>
              <div class="client-info">
                <span>Version: {{ client.version }}</span>
                <span>Lag: {{ client.lag }}ms</span>
              </div>
              <div class="client-last-seen">
                Last heartbeat: {{ new Date(client.lastHeartbeat).toLocaleTimeString() }}
              </div>
            </div>
            <div class="client-actions">
              <button class="btn-small">Force Sync</button>
              <button class="btn-small btn-danger">Disconnect</button>
            </div>
          </div>
        </div>
      </div>
      
      <div class="deltas-panel">
        <h3>Recent Deltas</h3>
        <div v-if="appStore.deltaStream.length === 0" class="empty-state">
          No deltas yet
        </div>
        <div v-else class="deltas-list">
          <div v-for="delta in appStore.deltaStream.slice(0, 10)" :key="delta.id" class="delta-item">
            <div class="delta-header">
              <span class="delta-type" :class="delta.type">{{ delta.type.toUpperCase() }}</span>
              <span class="delta-timestamp">{{ new Date(delta.timestamp).toLocaleTimeString() }}</span>
            </div>
            <div class="delta-details">
              <span class="delta-table">{{ delta.table }}</span>
              <span class="delta-entity">Entity: {{ delta.entityId }}</span>
              <span class="delta-size">{{ delta.size }} bytes</span>
            </div>
          </div>
        </div>
      </div>
      
      <div class="conflicts-panel">
        <h3>Recent Conflicts</h3>
        <div v-if="conflicts.length === 0" class="empty-state">
          No conflicts
        </div>
        <div v-else class="conflicts-list">
          <div v-for="conflict in conflicts.slice(0, 10)" :key="conflict.id" class="conflict-item">
            <div class="conflict-header">
              <span class="conflict-type">{{ conflict.type }}</span>
              <span class="conflict-resolution" :class="conflict.resolution">
                {{ conflict.resolution }}
              </span>
            </div>
            <div class="conflict-details">
              <span class="conflict-table">{{ conflict.table }}</span>
              <span class="conflict-entity">Entity: {{ conflict.entityId }}</span>
              <span class="conflict-timestamp">{{ new Date(conflict.timestamp).toLocaleTimeString() }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.replication-dashboard {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.dashboard-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: 1rem;
}

.server-controls {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.btn-success {
  background-color: #10b981;
  color: white;
  border-color: #10b981;
}

.btn-danger {
  background-color: #ef4444;
  color: white;
  border-color: #ef4444;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 1rem;
}

.stat-card {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 1rem;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background-color: var(--bg-primary);
}

.stat-icon {
  font-size: 2rem;
}

.stat-content {
  flex: 1;
}

.stat-value {
  font-size: 1.5rem;
  font-weight: 600;
  color: var(--text-primary);
}

.stat-label {
  font-size: 0.875rem;
  color: var(--text-secondary);
}

.dashboard-content {
  flex: 1;
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 1rem;
  height: calc(100vh - 300px);
}

.clients-panel, .deltas-panel, .conflicts-panel {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 1rem;
  background-color: var(--bg-secondary);
  overflow-y: auto;
}

.clients-panel h3, .deltas-panel h3, .conflicts-panel h3 {
  margin-top: 0;
  margin-bottom: 1rem;
  color: var(--text-primary);
}

.empty-state {
  color: var(--text-secondary);
  text-align: center;
  padding: 2rem;
}

.clients-list, .deltas-list, .conflicts-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.client-card {
  padding: 1rem;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background-color: var(--bg-primary);
}

.client-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.5rem;
}

.client-id {
  font-weight: 600;
  color: var(--text-primary);
}

.client-status {
  font-size: 0.75rem;
  padding: 0.125rem 0.5rem;
  border-radius: 1rem;
  text-transform: uppercase;
}

.client-status.connected {
  background-color: rgba(16, 185, 129, 0.1);
  color: #10b981;
}

.client-status.disconnected {
  background-color: rgba(239, 68, 68, 0.1);
  color: #ef4444;
}

.client-details {
  margin-bottom: 0.5rem;
}

.client-address {
  font-family: monospace;
  font-size: 0.875rem;
  color: var(--text-secondary);
  margin-bottom: 0.25rem;
}

.client-info {
  display: flex;
  justify-content: space-between;
  font-size: 0.75rem;
  color: var(--text-secondary);
}

.client-last-seen {
  font-size: 0.75rem;
  color: var(--text-secondary);
  margin-top: 0.25rem;
}

.client-actions {
  display: flex;
  gap: 0.5rem;
  margin-top: 0.5rem;
}

.btn-small {
  padding: 0.25rem 0.5rem;
  font-size: 0.75rem;
}

.delta-item, .conflict-item {
  padding: 0.75rem;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background-color: var(--bg-primary);
}

.delta-header, .conflict-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.25rem;
}

.delta-type, .conflict-type {
  font-size: 0.75rem;
  font-weight: 600;
  padding: 0.125rem 0.5rem;
  border-radius: 1rem;
  text-transform: uppercase;
}

.delta-type.insert {
  background-color: rgba(16, 185, 129, 0.1);
  color: #10b981;
}

.delta-type.update {
  background-color: rgba(59, 130, 246, 0.1);
  color: #3b82f6;
}

.delta-type.delete {
  background-color: rgba(239, 68, 68, 0.1);
  color: #ef4444;
}

.delta-timestamp, .conflict-timestamp {
  font-size: 0.75rem;
  color: var(--text-secondary);
}

.delta-details, .conflict-details {
  display: flex;
  justify-content: space-between;
  font-size: 0.75rem;
  color: var(--text-secondary);
}

.conflict-resolution {
  font-size: 0.75rem;
  padding: 0.125rem 0.5rem;
  border-radius: 1rem;
}

.conflict-resolution.server-wins {
  background-color: rgba(59, 130, 246, 0.1);
  color: #3b82f6;
}

.conflict-resolution.client-wins {
  background-color: rgba(16, 185, 129, 0.1);
  color: #10b981;
}
</style>