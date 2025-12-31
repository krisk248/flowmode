<script lang="ts">
  import { onMount } from 'svelte';
  import { fetchToday, fetchDetailed, fetchHistory, fetchAnalyticsSummary, fetchAnalyticsBurnout, getCategoryClass, getCategoryColor, fetchPomodoroStatus, startPomodoro, pausePomodoro, resumePomodoro, resetPomodoro, skipPomodoro } from './lib/api.js';

  // Svelte 5 state
  let activeTab = $state('summary');
  let today = $state<any>(null);
  let detailed = $state<any[]>([]);
  let history = $state<any[]>([]);
  let analytics = $state<any>(null);
  let burnout = $state<any>(null);
  let pomodoro = $state<any>(null);
  let loading = $state(true);
  let currentTime = $state(new Date());
  let lastDataHash = $state('');

  // Derived values
  let formattedDate = $derived(currentTime.toLocaleDateString('en-US', {
    weekday: 'long',
    year: 'numeric',
    month: 'long',
    day: 'numeric'
  }));

  let formattedTime = $derived(currentTime.toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  }));

  // Goals (in seconds)
  const SCREEN_GOAL = 8 * 3600;  // 8 hours
  const ACTIVE_GOAL = 6 * 3600;  // 6 hours active
  const FOCUS_GOAL = 2 * 3600;   // 2 hours deep focus
  const BREAK_GOAL = 4;          // 4 breaks

  let totalSecs = $derived(today?.total_secs || 0);
  let activeSecs = $derived(today?.active_secs || 0);
  let focusSecs = $derived((analytics?.focus_streak_mins || 0) * 60);

  // Ring progress values (0-100)
  let screenProgress = $derived(Math.min((totalSecs / SCREEN_GOAL) * 100, 100));
  let activeProgress = $derived(Math.min((activeSecs / ACTIVE_GOAL) * 100, 100));
  let focusProgress = $derived(Math.min((focusSecs / FOCUS_GOAL) * 100, 100));

  // Calculate idle time
  let idleSecs = $derived(calculateIdleTime(today, currentTime));
  let totalWithIdle = $derived((today?.active_secs || 0) + (today?.passive_secs || 0) + idleSecs);
  let activeWidth = $derived(totalWithIdle > 0 ? ((today?.active_secs || 0) / totalWithIdle) * 100 : 0);
  let passiveWidth = $derived(totalWithIdle > 0 ? ((today?.passive_secs || 0) / totalWithIdle) * 100 : 0);
  let idleWidth = $derived(totalWithIdle > 0 ? (idleSecs / totalWithIdle) * 100 : 0);

  function calculateIdleTime(todayData: any, now: Date) {
    if (!todayData?.hourly || todayData.hourly.length === 0) return 0;
    const hoursWithActivity = todayData.hourly
      .filter((h: any) => (h.active_secs || 0) + (h.passive_secs || 0) > 0)
      .map((h: any) => h.hour);
    if (hoursWithActivity.length === 0) return 0;
    const firstHour = Math.min(...hoursWithActivity);
    const currentHour = now.getHours();
    const currentMinute = now.getMinutes();
    const elapsedSecs = ((currentHour - firstHour) * 3600) + (currentMinute * 60);
    const trackedSecs = (todayData.active_secs || 0) + (todayData.passive_secs || 0);
    return Math.max(0, elapsedSecs - trackedSecs);
  }

  // Hourly chart data
  let hourlyData = $derived(
    Array.from({length: 24}, (_, hour) => {
      const h = today?.hourly?.find((x: any) => x.hour === hour);
      return {
        hour: hour.toString(),
        active: Math.round((h?.active_secs || 0) / 60),
        passive: Math.round((h?.passive_secs || 0) / 60),
        total: Math.round(((h?.active_secs || 0) + (h?.passive_secs || 0)) / 60)
      };
    })
  );

  let hourlyMax = $derived(Math.max(...hourlyData.map(d => d.total), 1));

  // Weekly trend data
  let weeklyData = $derived(
    history.slice(0, 7).reverse().map((day: any) => ({
      date: day.date,
      total: Math.round(day.total_secs / 3600 * 10) / 10
    }))
  );

  // Productivity radar data
  let radarData = $derived([
    { metric: 'Focus', value: Math.min((analytics?.focus_streak_mins || 0) / 60 * 100, 100), angle: 0 },
    { metric: 'Active', value: analytics?.active_percent || 0, angle: 72 },
    { metric: 'Consistency', value: history.length > 0 ? Math.min(history.length / 7 * 100, 100) : 0, angle: 144 },
    { metric: 'Balance', value: burnout?.level === 'low' ? 100 : burnout?.level === 'medium' ? 60 : burnout?.level === 'high' ? 30 : 10, angle: 216 },
    { metric: 'Deep Work', value: Math.min((analytics?.best_hour_secs || 0) / 3600 * 100, 100), angle: 288 }
  ]);

  // Average productivity score
  let avgScore = $derived(Math.round(radarData.reduce((s, d) => s + d.value, 0) / radarData.length));

  // App timeline data - group detailed entries by app with time ranges
  let appTimeline = $derived(() => {
    if (!today?.apps) return [];
    const currentHour = currentTime.getHours();
    return today.apps.slice(0, 10).map((app: any, idx: number) => {
      // Simulate time ranges based on hourly data
      const appHours = today.hourly?.filter((h: any) => h.hour <= currentHour) || [];
      const startHour = Math.max(0, currentHour - Math.floor(app.secs / 3600) - idx);
      const endHour = Math.min(23, startHour + Math.ceil(app.secs / 1800));
      return {
        name: app.name,
        category: app.category,
        secs: app.secs,
        formatted: app.formatted,
        startHour,
        endHour,
        color: getCategoryColor(app.category)
      };
    });
  });

  // Monthly pattern data for radial chart
  let monthlyPattern = $derived(() => {
    return history.slice(0, 30).map((day: any, idx: number) => ({
      date: day.date,
      hours: day.total_secs / 3600,
      angle: (idx / 30) * 360
    }));
  });

  // Create data hash for comparison (prevents unnecessary re-renders)
  function getDataHash(data: any) {
    return JSON.stringify({
      total: data?.total_secs,
      active: data?.active_secs,
      apps: data?.apps?.length
    });
  }

  async function loadData() {
    try {
      const newToday = await fetchToday();
      const newHash = getDataHash(newToday);

      if (newHash !== lastDataHash) {
        today = newToday;
        lastDataHash = newHash;
      }

      if (activeTab === 'pomodoro') {
        pomodoro = await fetchPomodoroStatus();
      } else if (activeTab === 'detailed') {
        detailed = await fetchDetailed();
      } else if (activeTab === 'history' || activeTab === 'patterns') {
        history = await fetchHistory();
      } else if (activeTab === 'analytics') {
        const [a, b, h] = await Promise.all([
          fetchAnalyticsSummary(),
          fetchAnalyticsBurnout(),
          fetchHistory()
        ]);
        analytics = a;
        burnout = b;
        history = h;
      }
      loading = false;
    } catch (e) {
      console.error('Failed to fetch data:', e);
    }
  }

  async function handlePomodoroStart() {
    await startPomodoro();
    pomodoro = await fetchPomodoroStatus();
  }

  async function handlePomodoroPause() {
    await pausePomodoro();
    pomodoro = await fetchPomodoroStatus();
  }

  async function handlePomodoroResume() {
    await resumePomodoro();
    pomodoro = await fetchPomodoroStatus();
  }

  async function handlePomodoroReset() {
    await resetPomodoro();
    pomodoro = await fetchPomodoroStatus();
  }

  async function handlePomodoroSkip() {
    await skipPomodoro();
    pomodoro = await fetchPomodoroStatus();
  }

  function getPomodoroStateLabel(state: string) {
    const labels: Record<string, string> = {
      'idle': 'Ready',
      'working': 'Working',
      'short_break': 'Short Break',
      'long_break': 'Long Break',
      'paused': 'Paused'
    };
    return labels[state] || state;
  }

  function getPomodoroStateColor(state: string) {
    const colors: Record<string, string> = {
      'idle': '#6e7681',
      'working': '#f85149',
      'short_break': '#3fb950',
      'long_break': '#58a6ff',
      'paused': '#d29922'
    };
    return colors[state] || '#6e7681';
  }

  function formatHour(hour: number | null | undefined) {
    if (hour === null || hour === undefined) return '--';
    return hour < 12 ? `${hour || 12}am` : `${hour === 12 ? 12 : hour - 12}pm`;
  }

  function formatDuration(secs: number) {
    if (!secs) return '0s';
    const hours = Math.floor(secs / 3600);
    const minutes = Math.floor((secs % 3600) / 60);
    if (hours > 0) return `${hours}h ${minutes}m`;
    if (minutes > 0) return `${minutes}m`;
    return `${secs}s`;
  }

  function getBurnoutColor(level: string) {
    const colors: Record<string, string> = {
      low: '#3fb950',
      medium: '#d29922',
      high: '#f97316',
      critical: '#f85149'
    };
    return colors[level] || '#6e7681';
  }

  function switchTab(tab: string) {
    activeTab = tab;
    loadData();
  }

  function groupDetailedByApp(items: any[]) {
    const groups: Record<string, any> = {};
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

  // Helper to create SVG arc path
  function describeArc(cx: number, cy: number, radius: number, startAngle: number, endAngle: number) {
    const start = polarToCartesian(cx, cy, radius, endAngle);
    const end = polarToCartesian(cx, cy, radius, startAngle);
    const largeArcFlag = endAngle - startAngle <= 180 ? "0" : "1";
    return `M ${start.x} ${start.y} A ${radius} ${radius} 0 ${largeArcFlag} 0 ${end.x} ${end.y}`;
  }

  function polarToCartesian(cx: number, cy: number, radius: number, angleInDegrees: number) {
    const angleInRadians = (angleInDegrees - 90) * Math.PI / 180;
    return {
      x: cx + (radius * Math.cos(angleInRadians)),
      y: cy + (radius * Math.sin(angleInRadians))
    };
  }

  // Radar chart helper
  function radarPoint(value: number, angle: number, maxRadius: number) {
    const radius = (value / 100) * maxRadius;
    const angleRad = (angle - 90) * Math.PI / 180;
    return {
      x: 100 + radius * Math.cos(angleRad),
      y: 100 + radius * Math.sin(angleRad)
    };
  }

  onMount(() => {
    loadData();

    const timeInterval = setInterval(() => {
      currentTime = new Date();
      if (activeTab === 'pomodoro') {
        fetchPomodoroStatus().then(p => pomodoro = p).catch(() => {});
      }
    }, 1000);

    const dataInterval = setInterval(loadData, 30000);

    return () => {
      clearInterval(timeInterval);
      clearInterval(dataInterval);
    };
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
  <button class="tab" class:active={activeTab === 'summary'} onclick={() => switchTab('summary')}>
    Summary
  </button>
  <button class="tab" class:active={activeTab === 'detailed'} onclick={() => switchTab('detailed')}>
    Timeline
  </button>
  <button class="tab" class:active={activeTab === 'analytics'} onclick={() => switchTab('analytics')}>
    Analytics
  </button>
  <button class="tab" class:active={activeTab === 'patterns'} onclick={() => switchTab('patterns')}>
    Patterns
  </button>
  <button class="tab" class:active={activeTab === 'pomodoro'} onclick={() => switchTab('pomodoro')}>
    Pomodoro
  </button>
  <button class="tab" class:active={activeTab === 'history'} onclick={() => switchTab('history')}>
    History
  </button>
</div>

{#if loading}
  <div class="loading">Loading...</div>

{:else if activeTab === 'summary'}
  <!-- Summary Tab with Concentric Rings -->
  <div class="summary-grid">
    <!-- Activity Quality Rings -->
    <div class="card rings-card">
      <div class="card-header">
        <span class="card-title">Activity Quality</span>
      </div>
      <div class="rings-container">
        <svg viewBox="0 0 200 200" class="rings-svg">
          <!-- Background rings -->
          <circle cx="100" cy="100" r="85" fill="none" stroke="#1a3d1a" stroke-width="12"/>
          <circle cx="100" cy="100" r="65" fill="none" stroke="#1a2a3d" stroke-width="12"/>
          <circle cx="100" cy="100" r="45" fill="none" stroke="#3d2a1a" stroke-width="12"/>

          <!-- Active Time Ring (Green/Outer) - shows % of total -->
          {#if activeWidth > 0}
            <path
              d={describeArc(100, 100, 85, 0, Math.max(activeWidth * 3.6, 5))}
              fill="none"
              stroke="#3fb950"
              stroke-width="12"
              stroke-linecap="round"
              class="ring-progress"
            />
          {/if}

          <!-- Passive Time Ring (Blue/Middle) - shows % of total -->
          {#if passiveWidth > 0}
            <path
              d={describeArc(100, 100, 65, 0, Math.max(passiveWidth * 3.6, 5))}
              fill="none"
              stroke="#58a6ff"
              stroke-width="12"
              stroke-linecap="round"
              class="ring-progress"
            />
          {/if}

          <!-- Idle Time Ring (Orange/Inner) - shows % of total -->
          {#if idleWidth > 0}
            <path
              d={describeArc(100, 100, 45, 0, Math.max(idleWidth * 3.6, 5))}
              fill="none"
              stroke="#d29922"
              stroke-width="12"
              stroke-linecap="round"
              class="ring-progress"
            />
          {/if}

          <!-- Center text -->
          <text x="100" y="95" text-anchor="middle" class="center-percent" fill="#e6edf3">{Math.round(activeWidth)}%</text>
          <text x="100" y="115" text-anchor="middle" class="center-label" fill="#8b949e">active</text>
        </svg>

        <div class="rings-legend">
          <div class="ring-item">
            <span class="ring-dot green"></span>
            <span class="ring-label">Active</span>
            <span class="ring-value">{formatDuration(today?.active_secs || 0)}</span>
            <span class="ring-percent">{Math.round(activeWidth)}%</span>
          </div>
          <div class="ring-item">
            <span class="ring-dot blue"></span>
            <span class="ring-label">Passive</span>
            <span class="ring-value">{formatDuration(today?.passive_secs || 0)}</span>
            <span class="ring-percent">{Math.round(passiveWidth)}%</span>
          </div>
          <div class="ring-item">
            <span class="ring-dot orange"></span>
            <span class="ring-label">Idle</span>
            <span class="ring-value">{formatDuration(idleSecs)}</span>
            <span class="ring-percent">{Math.round(idleWidth)}%</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Apps List -->
    <div class="card">
      <div class="card-header">
        <span class="card-title">Top Apps ({today?.apps?.length || 0})</span>
      </div>
      {#if today?.apps?.length > 0}
        <div class="app-list">
          {#each today.apps.slice(0, 6) as app}
            <div class="app-item">
              <div class="app-icon {getCategoryClass(app.category)}"></div>
              <span class="app-name">{app.name}</span>
              <span class="app-time">{app.formatted}</span>
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
          <p>No activity yet</p>
        </div>
      {/if}
    </div>
  </div>

  <!-- Hourly Activity Chart -->
  <div class="card">
    <div class="card-header">
      <span class="card-title">Today's Activity</span>
      <div class="chart-legend">
        <span class="legend-item"><span class="legend-dot active"></span> Active</span>
      </div>
    </div>
    <div class="hourly-chart">
      {#each hourlyData as bar, i}
        <div class="hour-bar-wrapper">
          <div
            class="hour-bar"
            style="height: {hourlyMax > 0 ? (bar.total / hourlyMax) * 100 : 0}%"
            title="{bar.hour}:00 - {bar.total}m"
          ></div>
          <span class="hour-label">{i % 3 === 0 ? bar.hour : ''}</span>
        </div>
      {/each}
    </div>
  </div>

  <!-- Time Breakdown -->
  <div class="card">
    <div class="card-header">
      <span class="card-title">Time Breakdown</span>
    </div>
    <div class="time-breakdown">
      <div class="breakdown-stacked-bar">
        <div class="stacked-fill active" style="width: {activeWidth}%"></div>
        <div class="stacked-fill passive" style="width: {passiveWidth}%"></div>
        <div class="stacked-fill idle" style="width: {idleWidth}%"></div>
      </div>
      <div class="breakdown-items">
        <div class="breakdown-item-row">
          <span class="breakdown-dot active"></span>
          <span class="breakdown-label">Active</span>
          <span class="breakdown-value">{formatDuration(today?.active_secs || 0)}</span>
          <span class="breakdown-percent">{Math.round(activeWidth)}%</span>
        </div>
        <div class="breakdown-item-row">
          <span class="breakdown-dot passive"></span>
          <span class="breakdown-label">Passive</span>
          <span class="breakdown-value">{formatDuration(today?.passive_secs || 0)}</span>
          <span class="breakdown-percent">{Math.round(passiveWidth)}%</span>
        </div>
        <div class="breakdown-item-row">
          <span class="breakdown-dot idle"></span>
          <span class="breakdown-label">Idle</span>
          <span class="breakdown-value">{formatDuration(idleSecs)}</span>
          <span class="breakdown-percent">{Math.round(idleWidth)}%</span>
        </div>
      </div>
    </div>
  </div>

{:else if activeTab === 'detailed'}
  <!-- Timeline Tab with Horizontal Bars -->
  <div class="card">
    <div class="card-header">
      <span class="card-title">App Usage Timeline</span>
    </div>
    <div class="timeline-container">
      <div class="timeline-header">
        {#each Array(24) as _, i}
          {#if i % 4 === 0}
            <span class="timeline-hour">{formatHour(i)}</span>
          {/if}
        {/each}
      </div>
      <div class="timeline-bars">
        {#each appTimeline() as app, idx}
          <div class="timeline-row">
            <span class="timeline-app-name">{app.name}</span>
            <div class="timeline-track">
              <div
                class="timeline-bar"
                style="left: {(app.startHour / 24) * 100}%; width: {((app.endHour - app.startHour) / 24) * 100}%; background: {app.color}"
                title="{app.name}: {app.formatted}"
              ></div>
            </div>
          </div>
        {/each}
      </div>
    </div>
  </div>

  <!-- Detailed Window Titles -->
  <div class="card">
    <div class="card-header">
      <span class="card-title">Window Details ({detailed.length})</span>
    </div>
    {#if detailed.length > 0}
      {#each groupDetailedByApp(detailed) as group}
        <div class="detailed-group">
          <div class="detailed-header">
            <div class="app-icon {getCategoryClass(group.category)}"></div>
            <span class="detailed-app-name">{group.app_name}</span>
          </div>
          {#each group.items.slice(0, 5) as item}
            <div class="detailed-item">
              <span class="detailed-time">{item.formatted}</span>
              <span class="detailed-title">{item.window_title}</span>
            </div>
          {/each}
        </div>
      {/each}
    {:else}
      <div class="empty">
        <p>No detailed activity yet</p>
      </div>
    {/if}
  </div>

{:else if activeTab === 'analytics'}
  <!-- Analytics Tab with Segmented Arc + Radar -->
  <div class="analytics-grid">
    <!-- Segmented Arc Productivity Score -->
    <div class="card">
      <div class="card-header">
        <span class="card-title">Productivity Score</span>
      </div>
      <div class="segmented-arc-container">
        <svg viewBox="0 0 200 200" class="segmented-arc">
          {#each Array(40) as _, i}
            {@const angle = (i / 40) * 360 - 90}
            {@const isActive = i < (avgScore / 100) * 40}
            {@const x1 = 100 + 70 * Math.cos(angle * Math.PI / 180)}
            {@const y1 = 100 + 70 * Math.sin(angle * Math.PI / 180)}
            {@const x2 = 100 + 85 * Math.cos(angle * Math.PI / 180)}
            {@const y2 = 100 + 85 * Math.sin(angle * Math.PI / 180)}
            <line
              {x1} {y1} {x2} {y2}
              stroke={isActive ? (avgScore > 70 ? '#3fb950' : avgScore > 40 ? '#d29922' : '#f85149') : '#21262d'}
              stroke-width="6"
              stroke-linecap="round"
              class="segment"
            />
          {/each}
          <text x="100" y="95" text-anchor="middle" class="score-number">{avgScore}</text>
          <text x="100" y="115" text-anchor="middle" class="score-label-text">Score</text>
        </svg>
      </div>
    </div>

    <!-- Radar Chart -->
    <div class="card">
      <div class="card-header">
        <span class="card-title">Performance Metrics</span>
      </div>
      <div class="radar-container">
        <svg viewBox="0 0 200 200" class="radar-svg">
          <!-- Grid circles -->
          <circle cx="100" cy="100" r="80" fill="none" stroke="#21262d" stroke-width="1"/>
          <circle cx="100" cy="100" r="60" fill="none" stroke="#21262d" stroke-width="1"/>
          <circle cx="100" cy="100" r="40" fill="none" stroke="#21262d" stroke-width="1"/>
          <circle cx="100" cy="100" r="20" fill="none" stroke="#21262d" stroke-width="1"/>

          <!-- Axis lines -->
          {#each radarData as point}
            {@const end = radarPoint(100, point.angle, 80)}
            <line x1="100" y1="100" x2={end.x} y2={end.y} stroke="#30363d" stroke-width="1"/>
          {/each}

          <!-- Data polygon -->
          <polygon
            points={radarData.map(p => {
              const pt = radarPoint(p.value, p.angle, 80);
              return `${pt.x},${pt.y}`;
            }).join(' ')}
            fill="rgba(88, 166, 255, 0.3)"
            stroke="#58a6ff"
            stroke-width="2"
          />

          <!-- Data points -->
          {#each radarData as point}
            {@const pt = radarPoint(point.value, point.angle, 80)}
            <circle cx={pt.x} cy={pt.y} r="4" fill="#58a6ff"/>
          {/each}

          <!-- Labels -->
          {#each radarData as point}
            {@const labelPt = radarPoint(115, point.angle, 80)}
            <text x={labelPt.x} y={labelPt.y} text-anchor="middle" class="radar-label">{point.metric}</text>
          {/each}
        </svg>
      </div>
    </div>
  </div>

  <!-- Metrics Breakdown -->
  <div class="card">
    <div class="card-header">
      <span class="card-title">Metrics Breakdown</span>
    </div>
    <div class="metrics-grid">
      {#each radarData as metric}
        <div class="metric-card">
          <div class="metric-header">
            <span class="metric-title">{metric.metric}</span>
            <span class="metric-score" style="color: {metric.value > 70 ? '#3fb950' : metric.value > 40 ? '#d29922' : '#f85149'}">{Math.round(metric.value)}</span>
          </div>
          <div class="metric-bar-bg">
            <div class="metric-bar-fill" style="width: {metric.value}%; background: {metric.value > 70 ? '#3fb950' : metric.value > 40 ? '#d29922' : '#f85149'}"></div>
          </div>
        </div>
      {/each}
    </div>
  </div>

  <!-- Insights -->
  <div class="card">
    <div class="card-header">
      <span class="card-title">Today's Insights</span>
    </div>
    <div class="insights-grid">
      <div class="insight-box">
        <span class="insight-icon">⏰</span>
        <span class="insight-label">Peak Hour</span>
        <span class="insight-value">{formatHour(analytics?.best_hour)}</span>
      </div>
      <div class="insight-box">
        <span class="insight-icon">🎯</span>
        <span class="insight-label">Top App</span>
        <span class="insight-value">{analytics?.most_used_app || 'None'}</span>
      </div>
      <div class="insight-box">
        <span class="insight-icon">🔥</span>
        <span class="insight-label">Focus Streak</span>
        <span class="insight-value">{analytics?.focus_streak_mins || 0}m</span>
      </div>
      <div class="insight-box">
        <span class="insight-icon">📈</span>
        <span class="insight-label">Active %</span>
        <span class="insight-value">{analytics?.active_percent || 0}%</span>
      </div>
    </div>
  </div>

{:else if activeTab === 'patterns'}
  <!-- Patterns Tab with Radial View -->
  <div class="card">
    <div class="card-header">
      <span class="card-title">Monthly Activity Pattern</span>
    </div>
    <div class="radial-container">
      <svg viewBox="0 0 300 300" class="radial-svg">
        <!-- Background circles -->
        <circle cx="150" cy="150" r="120" fill="none" stroke="#21262d" stroke-width="1"/>
        <circle cx="150" cy="150" r="90" fill="none" stroke="#21262d" stroke-width="1"/>
        <circle cx="150" cy="150" r="60" fill="none" stroke="#21262d" stroke-width="1"/>
        <circle cx="150" cy="150" r="30" fill="none" stroke="#21262d" stroke-width="1"/>

        <!-- Hour labels -->
        <text x="150" y="20" text-anchor="middle" class="radial-label">12h</text>
        <text x="280" y="155" text-anchor="middle" class="radial-label">8h</text>
        <text x="150" y="290" text-anchor="middle" class="radial-label">4h</text>
        <text x="20" y="155" text-anchor="middle" class="radial-label">0h</text>

        <!-- Data bars -->
        {#each monthlyPattern() as day, i}
          {@const angle = (i / 30) * 360 - 90}
          {@const maxHours = 12}
          {@const barLength = (day.hours / maxHours) * 90}
          {@const x1 = 150 + 30 * Math.cos(angle * Math.PI / 180)}
          {@const y1 = 150 + 30 * Math.sin(angle * Math.PI / 180)}
          {@const x2 = 150 + (30 + barLength) * Math.cos(angle * Math.PI / 180)}
          {@const y2 = 150 + (30 + barLength) * Math.sin(angle * Math.PI / 180)}
          <line
            {x1} {y1} {x2} {y2}
            stroke={day.hours > 8 ? '#f85149' : day.hours > 6 ? '#d29922' : '#3fb950'}
            stroke-width="8"
            stroke-linecap="round"
            class="radial-bar"
          />
        {/each}
      </svg>
      <div class="radial-legend">
        <span class="radial-legend-item"><span class="dot green"></span> &lt; 6h</span>
        <span class="radial-legend-item"><span class="dot yellow"></span> 6-8h</span>
        <span class="radial-legend-item"><span class="dot red"></span> &gt; 8h</span>
      </div>
    </div>
  </div>

  <!-- Weekly Trend -->
  <div class="card">
    <div class="card-header">
      <span class="card-title">Weekly Trend</span>
    </div>
    <div class="weekly-chart">
      {#if weeklyData.length > 0}
        {@const maxHours = Math.max(...weeklyData.map(d => d.total), 8)}
        {#each weeklyData as day}
          <div class="week-bar-wrapper">
            <div class="week-bar-bg">
              <div
                class="week-bar"
                style="height: {maxHours > 0 ? (day.total / maxHours) * 100 : 0}%"
                title="{day.date}: {day.total}h"
              ></div>
            </div>
            <span class="week-label">{day.date.slice(5)}</span>
            <span class="week-value">{day.total}h</span>
          </div>
        {/each}
      {:else}
        <div class="empty"><p>No history data</p></div>
      {/if}
    </div>
  </div>

  <!-- Burnout Analysis -->
  {#if burnout}
    {@const riskValue = burnout.level === 'low' ? 0.2 : burnout.level === 'medium' ? 0.5 : burnout.level === 'high' ? 0.75 : 0.95}
    <div class="card">
      <div class="card-header">
        <span class="card-title">Burnout Risk Analysis</span>
      </div>
      <div class="burnout-analysis">
        <div class="burnout-gauge">
          <svg viewBox="0 0 200 120" class="burnout-svg">
            <path d="M 20 100 A 80 80 0 0 1 180 100" fill="none" stroke="#21262d" stroke-width="16" stroke-linecap="round"/>
            <path d="M 20 100 A 80 80 0 0 1 180 100" fill="none" stroke={getBurnoutColor(burnout.level)} stroke-width="16" stroke-linecap="round" stroke-dasharray={`${riskValue * 251} 251`}/>
            <text x="100" y="90" text-anchor="middle" class="burnout-level" fill={getBurnoutColor(burnout.level)}>{burnout.level.toUpperCase()}</text>
          </svg>
        </div>
        <div class="burnout-stats">
          <div class="burnout-stat">
            <span class="stat-value">{burnout.weekly_hours.toFixed(1)}h</span>
            <span class="stat-label">This Week</span>
          </div>
          <div class="burnout-stat">
            <span class="stat-value">{burnout.consecutive_long_days}</span>
            <span class="stat-label">Long Days</span>
          </div>
          <div class="burnout-stat">
            <span class="stat-value">{burnout.trend_direction === 'increasing' ? '📈' : burnout.trend_direction === 'decreasing' ? '📉' : '➡️'}</span>
            <span class="stat-label">Trend</span>
          </div>
        </div>
        <div class="burnout-tip">
          💡 {burnout.recommendation}
        </div>
      </div>
    </div>
  {/if}

{:else if activeTab === 'pomodoro'}
  <div class="card pomodoro-tab">
    <div class="card-header">
      <span class="card-title">Pomodoro Timer</span>
      {#if pomodoro}
        <span class="pomodoro-count">{pomodoro.completed_pomodoros} completed today</span>
      {/if}
    </div>
    {#if pomodoro}
      <div class="pomodoro-widget">
        <div class="pomodoro-timer" style="--state-color: {getPomodoroStateColor(pomodoro.state)}">
          <div class="timer-circle large">
            <span class="timer-time">{pomodoro.remaining_formatted}</span>
            <span class="timer-state" style="color: {getPomodoroStateColor(pomodoro.state)}">{getPomodoroStateLabel(pomodoro.state)}</span>
          </div>
        </div>
        <div class="pomodoro-controls">
          {#if pomodoro.state === 'idle'}
            <button class="pomo-btn primary" onclick={handlePomodoroStart}>Start Work</button>
          {:else if pomodoro.state === 'paused'}
            <button class="pomo-btn primary" onclick={handlePomodoroResume}>Resume</button>
            <button class="pomo-btn" onclick={handlePomodoroReset}>Reset</button>
          {:else}
            <button class="pomo-btn" onclick={handlePomodoroPause}>Pause</button>
            <button class="pomo-btn" onclick={handlePomodoroSkip}>Skip</button>
            <button class="pomo-btn danger" onclick={handlePomodoroReset}>Reset</button>
          {/if}
        </div>
        <div class="pomodoro-info">
          <div class="pomo-info-item">
            <span class="pomo-info-label">Work</span>
            <span class="pomo-info-value">25m</span>
          </div>
          <div class="pomo-info-item">
            <span class="pomo-info-label">Short Break</span>
            <span class="pomo-info-value">5m</span>
          </div>
          <div class="pomo-info-item">
            <span class="pomo-info-label">Long Break</span>
            <span class="pomo-info-value">15m</span>
          </div>
        </div>
      </div>
    {:else}
      <div class="empty">
        <p>Loading timer...</p>
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
          <div class="history-day" style="--intensity: {Math.min(day.total_secs / 36000, 1)}">
            <div class="history-date">{day.date.slice(5)}</div>
            <div class="history-time">{day.formatted}</div>
          </div>
        {/each}
      </div>
    {:else}
      <div class="empty">
        <p>No history yet</p>
      </div>
    {/if}
  </div>
{/if}
