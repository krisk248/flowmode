const API_BASE = '/api';

export async function fetchToday() {
  const res = await fetch(`${API_BASE}/today`);
  return res.json();
}

export async function fetchDetailed() {
  const res = await fetch(`${API_BASE}/today/detailed`);
  return res.json();
}

export async function fetchHourly() {
  const res = await fetch(`${API_BASE}/today/hourly`);
  return res.json();
}

export async function fetchHistory() {
  const res = await fetch(`${API_BASE}/history`);
  return res.json();
}

export async function fetchAnalyticsSummary() {
  const res = await fetch(`${API_BASE}/analytics/summary`);
  return res.json();
}

export async function fetchAnalyticsTrends() {
  const res = await fetch(`${API_BASE}/analytics/trends`);
  return res.json();
}

export async function fetchAnalyticsBurnout() {
  const res = await fetch(`${API_BASE}/analytics/burnout`);
  return res.json();
}

export async function fetchStatus() {
  const res = await fetch(`${API_BASE}/status`);
  return res.json();
}

export async function pauseTracking() {
  const res = await fetch(`${API_BASE}/tracking/pause`, { method: 'POST' });
  return res.json();
}

export async function resumeTracking() {
  const res = await fetch(`${API_BASE}/tracking/resume`, { method: 'POST' });
  return res.json();
}

export function getCategoryClass(category) {
  const map = {
    'Development': 'cat-development',
    'Browser': 'cat-browser',
    'Terminal': 'cat-terminal',
    'Communication': 'cat-communication',
    'Notes': 'cat-notes',
    'Office': 'cat-office',
    'Files': 'cat-files',
  };
  return map[category] || 'cat-default';
}

export function getCategoryColor(category) {
  const map = {
    'Development': '#58a6ff',
    'Browser': '#3b82f6',
    'Terminal': '#3fb950',
    'Communication': '#a371f7',
    'Notes': '#d29922',
    'Office': '#f85149',
    'Files': '#94a3b8',
  };
  return map[category] || '#6e7681';
}

// Pomodoro API
export async function fetchPomodoroStatus() {
  const res = await fetch(`${API_BASE}/pomodoro/status`);
  return res.json();
}

export async function startPomodoro() {
  const res = await fetch(`${API_BASE}/pomodoro/start`, { method: 'POST' });
  return res.json();
}

export async function pausePomodoro() {
  const res = await fetch(`${API_BASE}/pomodoro/pause`, { method: 'POST' });
  return res.json();
}

export async function resumePomodoro() {
  const res = await fetch(`${API_BASE}/pomodoro/resume`, { method: 'POST' });
  return res.json();
}

export async function resetPomodoro() {
  const res = await fetch(`${API_BASE}/pomodoro/reset`, { method: 'POST' });
  return res.json();
}

export async function skipPomodoro() {
  const res = await fetch(`${API_BASE}/pomodoro/skip`, { method: 'POST' });
  return res.json();
}
