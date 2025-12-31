use anyhow::Result;
use chrono::{DateTime, Local, NaiveDate, Duration, Timelike};
use rusqlite::{Connection, params};
use std::path::Path;
use std::collections::HashMap;

/// Activity record
#[derive(Debug, Clone)]
pub struct ActivityRecord {
    pub id: i64,
    pub app_name: String,
    pub category: String,
    pub window_title: String,
    pub started_at: DateTime<Local>,
    pub ended_at: Option<DateTime<Local>>,
    pub duration_secs: i64,
}

/// Summary of activity for an app
#[derive(Debug, Clone)]
pub struct AppSummary {
    pub app_name: String,
    pub category: String,
    pub total_secs: i64,
    pub active_secs: i64,
    pub passive_secs: i64,
}

/// Hourly breakdown
#[derive(Debug, Clone)]
pub struct HourlyActivity {
    pub hour: u32,
    pub total_secs: i64,
}

/// Hourly breakdown with active/passive detail
#[derive(Debug, Clone)]
pub struct HourlyActivityDetailed {
    pub hour: u32,
    pub active_secs: i64,
    pub passive_secs: i64,
}

/// Database for storing activity
pub struct Storage {
    conn: Connection,
}

impl Storage {
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        let conn = Connection::open(path)?;

        // Create tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS activity (
                id INTEGER PRIMARY KEY,
                app_name TEXT NOT NULL,
                category TEXT NOT NULL,
                window_title TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                duration_secs INTEGER DEFAULT 0
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_activity_started
             ON activity(started_at)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_activity_app
             ON activity(app_name)",
            [],
        )?;

        // v0.5.0 Migration: Add active_secs and passive_secs columns
        let has_active_secs: bool = conn
            .prepare("SELECT active_secs FROM activity LIMIT 1")
            .is_ok();

        if !has_active_secs {
            // Add new columns
            conn.execute("ALTER TABLE activity ADD COLUMN active_secs INTEGER DEFAULT 0", [])?;
            conn.execute("ALTER TABLE activity ADD COLUMN passive_secs INTEGER DEFAULT 0", [])?;
            // Backfill: assume all existing time was active
            conn.execute("UPDATE activity SET active_secs = duration_secs WHERE active_secs = 0 AND duration_secs > 0", [])?;
        }

        Ok(Self { conn })
    }

    /// Start a new activity session
    pub fn start_activity(&self, app_name: &str, category: &str, window_title: &str) -> Result<i64> {
        let now = Local::now();
        self.conn.execute(
            "INSERT INTO activity (app_name, category, window_title, started_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![app_name, category, window_title, now.to_rfc3339()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// End an activity session
    pub fn end_activity(&self, id: i64) -> Result<()> {
        let now = Local::now();
        self.conn.execute(
            "UPDATE activity
             SET ended_at = ?1,
                 duration_secs = CAST((julianday(?1) - julianday(started_at)) * 86400 AS INTEGER)
             WHERE id = ?2",
            params![now.to_rfc3339(), id],
        )?;
        Ok(())
    }

    /// Update activity time counters (active_secs or passive_secs)
    /// Called periodically during tracking to increment the appropriate counter
    pub fn update_activity_time(&self, id: i64, active_delta: i64, passive_delta: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE activity
             SET active_secs = active_secs + ?1,
                 passive_secs = passive_secs + ?2,
                 duration_secs = duration_secs + ?1 + ?2
             WHERE id = ?3",
            params![active_delta, passive_delta, id],
        )?;
        Ok(())
    }

    /// Get current active session (if any)
    pub fn get_active_session(&self) -> Result<Option<ActivityRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, app_name, category, window_title, started_at, ended_at, duration_secs
             FROM activity WHERE ended_at IS NULL
             ORDER BY started_at DESC LIMIT 1"
        )?;

        let result = stmt.query_row([], |row| {
            let started_str: String = row.get(4)?;
            let ended_str: Option<String> = row.get(5)?;

            Ok(ActivityRecord {
                id: row.get(0)?,
                app_name: row.get(1)?,
                category: row.get(2)?,
                window_title: row.get(3)?,
                started_at: DateTime::parse_from_rfc3339(&started_str)
                    .map(|dt| dt.with_timezone(&Local))
                    .unwrap_or_else(|_| Local::now()),
                ended_at: ended_str.and_then(|s|
                    DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&Local))
                        .ok()
                ),
                duration_secs: row.get(6)?,
            })
        });

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get activity summary for today
    pub fn get_today_summary(&self) -> Result<Vec<AppSummary>> {
        let today = Local::now().date_naive();
        self.get_date_summary(today)
    }

    /// Get activity summary for a specific date
    pub fn get_date_summary(&self, date: NaiveDate) -> Result<Vec<AppSummary>> {
        let start = date.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Local).unwrap();
        let end = start + Duration::days(1);

        let mut stmt = self.conn.prepare(
            "SELECT app_name, category, SUM(duration_secs) as total,
                    SUM(active_secs) as active, SUM(passive_secs) as passive
             FROM activity
             WHERE started_at >= ?1 AND started_at < ?2
             GROUP BY app_name, category
             ORDER BY total DESC"
        )?;

        let rows = stmt.query_map(
            params![start.to_rfc3339(), end.to_rfc3339()],
            |row| {
                Ok(AppSummary {
                    app_name: row.get(0)?,
                    category: row.get(1)?,
                    total_secs: row.get(2)?,
                    active_secs: row.get(3)?,
                    passive_secs: row.get(4)?,
                })
            }
        )?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row?);
        }
        Ok(summaries)
    }

    /// Get total tracked time for today
    pub fn get_today_total_secs(&self) -> Result<i64> {
        let today = Local::now().date_naive();
        let start = today.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Local).unwrap();
        let end = start + Duration::days(1);

        let total: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(duration_secs), 0)
             FROM activity
             WHERE started_at >= ?1 AND started_at < ?2",
            params![start.to_rfc3339(), end.to_rfc3339()],
            |row| row.get(0),
        )?;

        Ok(total)
    }

    /// Get hourly breakdown for today
    pub fn get_today_hourly(&self) -> Result<Vec<HourlyActivity>> {
        let today = Local::now().date_naive();
        let start = today.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Local).unwrap();
        let end = start + Duration::days(1);

        let mut stmt = self.conn.prepare(
            "SELECT started_at, duration_secs
             FROM activity
             WHERE started_at >= ?1 AND started_at < ?2"
        )?;

        let rows = stmt.query_map(
            params![start.to_rfc3339(), end.to_rfc3339()],
            |row| {
                let started_str: String = row.get(0)?;
                let duration: i64 = row.get(1)?;
                Ok((started_str, duration))
            }
        )?;

        let mut hourly: HashMap<u32, i64> = HashMap::new();
        for row in rows {
            let (started_str, duration) = row?;
            if let Ok(dt) = DateTime::parse_from_rfc3339(&started_str) {
                let hour = dt.hour();
                *hourly.entry(hour).or_insert(0) += duration;
            }
        }

        let mut result: Vec<HourlyActivity> = hourly
            .into_iter()
            .map(|(hour, total_secs)| HourlyActivity { hour, total_secs })
            .collect();
        result.sort_by_key(|h| h.hour);

        Ok(result)
    }

    /// Get hourly breakdown with active/passive detail for today
    pub fn get_today_hourly_detailed(&self) -> Result<Vec<HourlyActivityDetailed>> {
        let today = Local::now().date_naive();
        let start = today.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Local).unwrap();
        let end = start + Duration::days(1);

        let mut stmt = self.conn.prepare(
            "SELECT started_at, active_secs, passive_secs
             FROM activity
             WHERE started_at >= ?1 AND started_at < ?2"
        )?;

        let rows = stmt.query_map(
            params![start.to_rfc3339(), end.to_rfc3339()],
            |row| {
                let started_str: String = row.get(0)?;
                let active: i64 = row.get(1)?;
                let passive: i64 = row.get(2)?;
                Ok((started_str, active, passive))
            }
        )?;

        let mut hourly_active: HashMap<u32, i64> = HashMap::new();
        let mut hourly_passive: HashMap<u32, i64> = HashMap::new();

        for row in rows {
            let (started_str, active, passive) = row?;
            if let Ok(dt) = DateTime::parse_from_rfc3339(&started_str) {
                let hour = dt.hour();
                *hourly_active.entry(hour).or_insert(0) += active;
                *hourly_passive.entry(hour).or_insert(0) += passive;
            }
        }

        // Combine into result for all hours that have data
        let mut hours: std::collections::HashSet<u32> = hourly_active.keys().copied().collect();
        hours.extend(hourly_passive.keys().copied());

        let mut result: Vec<HourlyActivityDetailed> = hours
            .into_iter()
            .map(|hour| HourlyActivityDetailed {
                hour,
                active_secs: *hourly_active.get(&hour).unwrap_or(&0),
                passive_secs: *hourly_passive.get(&hour).unwrap_or(&0),
            })
            .collect();
        result.sort_by_key(|h| h.hour);

        Ok(result)
    }

    /// Get week summary (last 7 days)
    pub fn get_week_summary(&self) -> Result<HashMap<NaiveDate, i64>> {
        let today = Local::now().date_naive();
        let week_ago = today - Duration::days(7);
        let start = week_ago.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Local).unwrap();
        let end = today.and_hms_opt(23, 59, 59).unwrap().and_local_timezone(Local).unwrap();

        let mut stmt = self.conn.prepare(
            "SELECT date(started_at) as day, SUM(duration_secs) as total
             FROM activity
             WHERE started_at >= ?1 AND started_at <= ?2
             GROUP BY day
             ORDER BY day"
        )?;

        let rows = stmt.query_map(
            params![start.to_rfc3339(), end.to_rfc3339()],
            |row| {
                let day_str: String = row.get(0)?;
                let total: i64 = row.get(1)?;
                Ok((day_str, total))
            }
        )?;

        let mut summary = HashMap::new();
        for row in rows {
            let (day_str, total) = row?;
            if let Ok(date) = NaiveDate::parse_from_str(&day_str, "%Y-%m-%d") {
                summary.insert(date, total);
            }
        }

        Ok(summary)
    }

    /// Close any open sessions (cleanup on shutdown)
    pub fn close_open_sessions(&self) -> Result<()> {
        let now = Local::now();
        self.conn.execute(
            "UPDATE activity
             SET ended_at = ?1,
                 duration_secs = CAST((julianday(?1) - julianday(started_at)) * 86400 AS INTEGER)
             WHERE ended_at IS NULL",
            params![now.to_rfc3339()],
        )?;
        Ok(())
    }

    /// Reset today's data (delete all entries from today)
    pub fn reset_today(&self) -> Result<()> {
        let today = Local::now().date_naive();
        let start = today.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Local).unwrap();
        let end = start + Duration::days(1);

        self.conn.execute(
            "DELETE FROM activity WHERE started_at >= ?1 AND started_at < ?2",
            params![start.to_rfc3339(), end.to_rfc3339()],
        )?;

        Ok(())
    }

    /// Get detailed activity (with window titles) for today
    pub fn get_today_detailed(&self) -> Result<Vec<(String, String, String, i64)>> {
        let today = Local::now().date_naive();
        let start = today.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Local).unwrap();
        let end = start + Duration::days(1);

        let mut stmt = self.conn.prepare(
            "SELECT app_name, category, window_title, SUM(duration_secs) as total
             FROM activity
             WHERE started_at >= ?1 AND started_at < ?2
             GROUP BY app_name, window_title
             HAVING total >= 5
             ORDER BY app_name, total DESC"
        )?;

        let rows = stmt.query_map(
            params![start.to_rfc3339(), end.to_rfc3339()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            }
        )?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Get history for the last N days
    pub fn get_history_days(&self, days: i64) -> Result<Vec<(NaiveDate, i64)>> {
        let today = Local::now().date_naive();
        let start_date = today - Duration::days(days);

        let mut stmt = self.conn.prepare(
            "SELECT date(started_at) as day, SUM(duration_secs) as total
             FROM activity
             WHERE date(started_at) >= date(?1)
             GROUP BY day
             ORDER BY day DESC"
        )?;

        let rows = stmt.query_map(
            params![start_date.to_string()],
            |row| {
                let date_str: String = row.get(0)?;
                let total: i64 = row.get(1)?;
                Ok((date_str, total))
            }
        )?;

        let mut results = Vec::new();
        for row in rows {
            let (date_str, total) = row?;
            if let Ok(date) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                results.push((date, total));
            }
        }
        Ok(results)
    }
}
