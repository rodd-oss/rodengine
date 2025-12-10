<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'

const cpuUsage = ref(50)
const memoryUsage = ref(65)
const activeBenchmark = ref<string>('')
const benchmarkResults = ref<any[]>([])
const metricsHistory = ref<any[]>([])

const benchmarks = [
  { id: 'insert', name: 'Insert Latency', description: 'Measure single insert operations' },
  { id: 'update', name: 'Update Latency', description: 'Measure single update operations' },
  { id: 'read', name: 'Read Latency', description: 'Measure single read operations' },
  { id: 'query', name: 'Query Throughput', description: 'Measure sequential table scans' },
  { id: 'replication', name: 'Replication Lag', description: 'Measure delta broadcast latency' }
]

const startBenchmark = (benchmarkId: string) => {
  activeBenchmark.value = benchmarkId
  // Simulate benchmark running
  setTimeout(() => {
    const result = {
      id: benchmarkId,
      timestamp: new Date().toISOString(),
      opsPerSec: Math.floor(Math.random() * 1000000),
      latency: Math.random() * 100,
      memoryUsage: Math.random() * 100
    }
    benchmarkResults.value.unshift(result)
    activeBenchmark.value = ''
  }, 2000)
}

const clearResults = () => {
  benchmarkResults.value = []
}

const collectMetrics = () => {
  const metric = {
    timestamp: Date.now(),
    cpu: Math.random() * 100,
    memory: Math.random() * 100,
    entities: Math.floor(Math.random() * 10000)
  }
  metricsHistory.value.push(metric)
  if (metricsHistory.value.length > 100) {
    metricsHistory.value.shift()
  }
}

let metricsInterval: number

onMounted(() => {
  metricsInterval = setInterval(collectMetrics, 1000)
  // Initial data
  for (let i = 0; i < 50; i++) {
    collectMetrics()
  }
})

onUnmounted(() => {
  clearInterval(metricsInterval)
})
</script>

<template>
  <div class="performance-profiling">
    <div class="profiling-header">
      <h2>Performance Profiling</h2>
      <div class="profiling-actions">
        <button class="btn" @click="clearResults">
          Clear Results
        </button>
      </div>
    </div>
    
    <div class="metrics-dashboard">
      <div class="metric-card">
        <div class="metric-header">
          <h3>CPU Usage</h3>
          <span class="metric-value">{{ cpuUsage.toFixed(1) }}%</span>
        </div>
        <div class="metric-chart">
          <div class="chart-bar" :style="{ width: `${cpuUsage}%` }"></div>
        </div>
      </div>
      
      <div class="metric-card">
        <div class="metric-header">
          <h3>Memory Usage</h3>
          <span class="metric-value">{{ memoryUsage.toFixed(1) }}%</span>
        </div>
        <div class="metric-chart">
          <div class="chart-bar" :style="{ width: `${memoryUsage}%` }"></div>
        </div>
      </div>
      
      <div class="metric-card">
        <div class="metric-header">
          <h3>Active Entities</h3>
          <span class="metric-value">{{ metricsHistory[metricsHistory.length - 1]?.entities || 0 }}</span>
        </div>
        <div class="metric-chart">
          <div class="chart-sparkline">
            <svg width="100%" height="40" viewBox="0 0 100 20">
              <polyline 
                fill="none" 
                stroke="#396cd8" 
                stroke-width="2" 
                :points="metricsHistory.map((m, i) => `${(i / metricsHistory.length) * 100},${20 - (m.entities / 10000) * 20}`).join(' ')"
              />
            </svg>
          </div>
        </div>
      </div>
    </div>
    
    <div class="benchmarks-section">
      <h3>Benchmarks</h3>
      <div class="benchmarks-grid">
        <div 
          v-for="benchmark in benchmarks" 
          :key="benchmark.id"
          class="benchmark-card"
          :class="{ 'running': activeBenchmark === benchmark.id }"
        >
          <div class="benchmark-info">
            <h4>{{ benchmark.name }}</h4>
            <p>{{ benchmark.description }}</p>
          </div>
          <button 
            class="btn"
            :class="{ 'btn-primary': activeBenchmark !== benchmark.id, 'btn-secondary': activeBenchmark === benchmark.id }"
            :disabled="activeBenchmark !== '' && activeBenchmark !== benchmark.id"
            @click="startBenchmark(benchmark.id)"
          >
            {{ activeBenchmark === benchmark.id ? 'Running...' : 'Run' }}
          </button>
        </div>
      </div>
    </div>
    
    <div class="results-section">
      <h3>Benchmark Results</h3>
      <div v-if="benchmarkResults.length === 0" class="empty-state">
        No benchmark results yet. Run a benchmark to see results.
      </div>
      <div v-else class="results-table">
        <div class="table-header">
          <div class="table-cell">Benchmark</div>
          <div class="table-cell">Timestamp</div>
          <div class="table-cell">Ops/sec</div>
          <div class="table-cell">Latency (μs)</div>
          <div class="table-cell">Memory Usage</div>
        </div>
        <div v-for="result in benchmarkResults" :key="result.timestamp" class="table-row">
          <div class="table-cell">{{ result.id }}</div>
          <div class="table-cell">{{ new Date(result.timestamp).toLocaleTimeString() }}</div>
          <div class="table-cell">{{ result.opsPerSec.toLocaleString() }}</div>
          <div class="table-cell">{{ result.latency.toFixed(2) }}</div>
          <div class="table-cell">{{ result.memoryUsage.toFixed(1) }}%</div>
        </div>
      </div>
    </div>
    
    <div class="charts-section">
      <h3>Real-time Metrics</h3>
      <div class="charts-grid">
        <div class="chart-card">
          <h4>CPU Usage Over Time</h4>
          <div class="chart-container">
            <svg width="100%" height="150" viewBox="0 0 100 50">
              <polyline 
                fill="none" 
                stroke="#10b981" 
                stroke-width="2" 
                :points="metricsHistory.map((m, i) => `${(i / metricsHistory.length) * 100},${50 - m.cpu * 0.5}`).join(' ')"
              />
            </svg>
          </div>
        </div>
        
        <div class="chart-card">
          <h4>Memory Usage Over Time</h4>
          <div class="chart-container">
            <svg width="100%" height="150" viewBox="0 0 100 50">
              <polyline 
                fill="none" 
                stroke="#ef4444" 
                stroke-width="2" 
                :points="metricsHistory.map((m, i) => `${(i / metricsHistory.length) * 100},${50 - m.memory * 0.5}`).join(' ')"
              />
            </svg>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.performance-profiling {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  overflow-y: auto;
}

.profiling-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.metrics-dashboard {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 1rem;
}

.metric-card {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 1rem;
  background-color: var(--bg-primary);
}

.metric-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.5rem;
}

.metric-header h3 {
  margin: 0;
  font-size: 1rem;
  color: var(--text-primary);
}

.metric-value {
  font-size: 1.5rem;
  font-weight: 600;
  color: var(--text-primary);
}

.metric-chart {
  height: 40px;
  background-color: var(--bg-secondary);
  border-radius: 4px;
  overflow: hidden;
}

.chart-bar {
  height: 100%;
  background-color: #396cd8;
  transition: width 0.3s ease;
}

.chart-sparkline {
  height: 100%;
}

.benchmarks-section {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 1rem;
  background-color: var(--bg-secondary);
}

.benchmarks-section h3 {
  margin-top: 0;
  margin-bottom: 1rem;
  color: var(--text-primary);
}

.benchmarks-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
  gap: 1rem;
}

.benchmark-card {
  padding: 1rem;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background-color: var(--bg-primary);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
}

.benchmark-card.running {
  border-color: #396cd8;
  background-color: rgba(57, 108, 216, 0.1);
}

.benchmark-info h4 {
  margin: 0 0 0.5rem 0;
  color: var(--text-primary);
}

.benchmark-info p {
  margin: 0;
  font-size: 0.875rem;
  color: var(--text-secondary);
}

.results-section {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 1rem;
  background-color: var(--bg-secondary);
}

.results-section h3 {
  margin-top: 0;
  margin-bottom: 1rem;
  color: var(--text-primary);
}

.empty-state {
  color: var(--text-secondary);
  text-align: center;
  padding: 2rem;
}

.results-table {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  overflow: hidden;
  background-color: var(--bg-primary);
}

.table-header {
  display: flex;
  background-color: var(--bg-secondary);
  font-weight: 600;
  color: var(--text-primary);
}

.table-row {
  display: flex;
  border-top: 1px solid var(--border-color);
}

.table-row:hover {
  background-color: var(--hover-color);
}

.table-cell {
  flex: 1;
  padding: 0.75rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.charts-section {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 1rem;
  background-color: var(--bg-secondary);
}

.charts-section h3 {
  margin-top: 0;
  margin-bottom: 1rem;
  color: var(--text-primary);
}

.charts-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(400px, 1fr));
  gap: 1rem;
}

.chart-card {
  border: 1px solid var(--border-color);
  border-radius: 4px;
  padding: 1rem;
  background-color: var(--bg-primary);
}

.chart-card h4 {
  margin: 0 0 1rem 0;
  color: var(--text-primary);
}

.chart-container {
  height: 150px;
  background-color: var(--bg-secondary);
  border-radius: 4px;
  padding: 0.5rem;
}
</style>