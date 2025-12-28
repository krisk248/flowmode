<script>
  import { onMount, onDestroy } from 'svelte';
  import { fetchToday, fetchDetailed, fetchHistory, getCategoryClass, getCategoryColor } from './lib/api.js';
  import Chart from 'chart.js/auto';

  let activeTab = 'summary';
  let today = null;
  let detailed = [];
  let history = [];
  let loading = true;
  let currentTime = new Date();
  let hourlyChart = null;
  let chartCanvas;
  let interval;

  $: formattedDate = currentTime.toLocaleDateString('en-US', {
    weekday: 'long',
    year: 'numeric',
    month: 'long',
    day: 'numeric'
  });

  $: formattedTime = currentTime.toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  });

  $: progress = today ? Math.min((today.total_secs / (8 * 3600)) * 100, 100) : 0;

  async function loadData() {
    try {
      today = await fetchToday();
      if (activeTab === 'detailed') {
        detailed = await fetchDetailed();
      } else if (activeTab === 'history') {
        history = await fetchHistory();
      }
      loading = false;

      if (activeTab === 'summary' && chartCanvas && today) {
        updateChart();
      }
    } catch (e) {
      console.error('Failed to fetch data:', e);
    }
  }

  function updateChart() {
    const ctx = chartCanvas.getContext('2d');

    const hourlyData = new Array(24).fill(0);
    if (today?.hourly) {
      today.hourly.forEach(h => {
        hourlyData[h.hour] = Math.round(h.secs / 60);
      });
    }

    if (hourlyChart) {
      hourlyChart.data.datasets[0].data = hourlyData;
      hourlyChart.update();
    } else {
      hourlyChart = new Chart(ctx, {
        type: 'bar',
        data: {
          labels: Array.from({length: 24}, (_, i) => `${i}:00`),
          datasets: [{
            label: 'Minutes',
            data: hourlyData,
            backgroundColor: '#58a6ff',
            borderRadius: 4,
          }]
        },
        options: {
          responsive: true,
          maintainAspectRatio: false,
          plugins: {
            legend: { display: false }
          },
          scales: {
            x: {
              grid: { display: false },
              ticks: { color: '#8b949e', font: { size: 10 } }
            },
            y: {
              grid: { color: '#30363d' },
              ticks: { color: '#8b949e' }
            }
          }
        }
      });
    }
  }

  function switchTab(tab) {
    activeTab = tab;
    loadData();
  }

  function groupDetailedByApp(items) {
    const groups = {};
    items.forEach(item => {
      if (!groups[item.app_name]) {
        groups[item.app_name] = {
          app_name: item.app_name,
          category: item.category,
          items: []
        };
      }
      groups[item.app_name].items.push(item);
    });
    return Object.values(groups);
  }

  onMount(() => {
    loadData();

    // Update time every second
    interval = setInterval(() => {
      currentTime = new Date();
    }, 1000);

    // Refresh data every 5 seconds
    const dataInterval = setInterval(loadData, 5000);

    return () => {
      clearInterval(dataInterval);
    };
  });

  onDestroy(() => {
    if (interval) clearInterval(interval);
    if (hourlyChart) hourlyChart.destroy();
  });
</script>

<div class="header">
  <div class="logo">
    <svg viewBox="0 0 100 100">
      <circle cx="50" cy="50" r="45" fill="#1a1a2e" stroke="#58a6ff" stroke-width="3"/>
      <circle cx="50" cy="50" r="35" fill="none" stroke="#58a6ff" stroke-width="2"/>
      <line x1="50" y1="50" x2="50" y2="25" stroke="#58a6ff" stroke-width="3" stroke-linecap="round"/>
      <line x1="50" y1="50" x2="70" y2="60" stroke="#f72585" stroke-width="2" stroke-linecap="round"/>
      <circle cx="50" cy="50" r="4" fill="#58a6ff"/>
    </svg>
    <h1>FlowMode</h1>
  </div>

  <div class="date-time">
    <div class="date">{formattedDate}</div>
    <div class="time">{formattedTime}</div>
  </div>
</div>

<div class="tabs">
  <button class="tab" class:active={activeTab === 'summary'} on:click={() => switchTab('summary')}>
    Summary
  </button>
  <button class="tab" class:active={activeTab === 'detailed'} on:click={() => switchTab('detailed')}>
    Detailed
  </button>
  <button class="tab" class:active={activeTab === 'history'} on:click={() => switchTab('history')}>
    History
  </button>
</div>

{#if loading}
  <div class="loading">Loading...</div>
{:else if activeTab === 'summary'}
  <!-- Progress Card -->
  <div class="card progress-container">
    <div class="progress-header">
      <div>
        <div class="progress-time">{today?.total_formatted || '0m'}</div>
        <div class="progress-target">of 8h daily target</div>
      </div>
      <div class="status">
        <span class="status-dot active"></span>
        <span>Tracking</span>
      </div>
    </div>
    <div class="progress-bar">
      <div class="progress-fill" style="width: {progress}%"></div>
    </div>
  </div>

  <div class="grid">
    <!-- App Breakdown -->
    <div class="card">
      <div class="card-header">
        <span class="card-title">Apps ({today?.apps?.length || 0})</span>
      </div>

      {#if today?.apps?.length > 0}
        <div class="app-list">
          {#each today.apps as app}
            <div class="app-item">
              <div class="app-icon {getCategoryClass(app.category)}"></div>
              <span class="app-name">{app.name}</span>
              <span class="app-time">{app.formatted}</span>
              <span class="app-percent">{app.percent}%</span>
              <div class="app-bar-container">
                <div
                  class="app-bar"
                  style="width: {app.percent}%; background: {getCategoryColor(app.category)}"
                ></div>
              </div>
            </div>
          {/each}
        </div>
      {:else}
        <div class="empty">
          <div class="empty-icon">📊</div>
          <p>No activity recorded yet</p>
        </div>
      {/if}
    </div>

    <!-- Hourly Chart -->
    <div class="card">
      <div class="card-header">
        <span class="card-title">Hourly Activity</span>
      </div>
      <div class="chart-container">
        <canvas bind:this={chartCanvas}></canvas>
      </div>
    </div>
  </div>

{:else if activeTab === 'detailed'}
  <div class="card">
    <div class="card-header">
      <span class="card-title">Window Titles ({detailed.length} entries)</span>
    </div>

    {#if detailed.length > 0}
      {#each groupDetailedByApp(detailed) as group}
        <div class="detailed-group">
          <div class="detailed-header">
            <div class="app-icon {getCategoryClass(group.category)}"></div>
            <span class="detailed-app-name">{group.app_name}</span>
          </div>
          {#each group.items as item}
            <div class="detailed-item">
              <span class="detailed-time">{item.formatted}</span>
              <span class="detailed-title">{item.window_title}</span>
            </div>
          {/each}
        </div>
      {/each}
    {:else}
      <div class="empty">
        <div class="empty-icon">📝</div>
        <p>No detailed activity yet</p>
      </div>
    {/if}
  </div>

{:else if activeTab === 'history'}
  <div class="card">
    <div class="card-header">
      <span class="card-title">Last 30 Days</span>
    </div>

    {#if history.length > 0}
      <div class="history-grid">
        {#each history as day}
          <div class="history-day">
            <div class="history-date">{day.date}</div>
            <div class="history-time">{day.formatted}</div>
          </div>
        {/each}
      </div>
    {:else}
      <div class="empty">
        <div class="empty-icon">📅</div>
        <p>No history yet</p>
      </div>
    {/if}
  </div>
{/if}
