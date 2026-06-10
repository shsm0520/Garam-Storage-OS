//! System Resource Monitor (CPU / RAM) via Linux /proc filesystem
use serde::{Serialize, Deserialize};
use std::str::FromStr;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SysMetrics {
    pub cpu_usage_pct: f32,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub ram_usage_pct: f32,
}

pub struct SystemMonitor;

// 🧠 [글로벌 안보 전광판]: 비동기 안전 토키오 락 뮤텍스 장착
lazy_static::lazy_static! {
    static ref LAST_JIFFIES: Mutex<(u64, u64)> = Mutex::new((0, 0));
}

impl SystemMonitor {
    /// 👑 [0ms 레이턴시 + 순수 비동기 I/O 완공]: 커널 스레드를 절대로 블로킹하지 않는 청정 저격선
    pub async fn fetch_metrics() -> SysMetrics {
        // --- 1. 🐑 [RAM 영역]: 토키오 퓨어 비동기 파일 하이재킹 ---
        let mem_info = tokio::fs::read_to_string("/proc/meminfo").await.unwrap_or_default();
        let mut total_kb = 0;
        let mut available_kb = 0;

        for line in mem_info.lines() {
            if line.contains("MemTotal:") {
                total_kb = line.split_whitespace().nth(1).and_then(|s| u64::from_str(s).ok()).unwrap_or(0);
            }
            if line.contains("MemAvailable:") {
                available_kb = line.split_whitespace().nth(1).and_then(|s| u64::from_str(s).ok()).unwrap_or(0);
            }
        }
        let total_mb = total_kb / 1024;
        let available_mb = available_kb / 1024;
        let used_mb = total_mb.saturating_sub(available_mb);
        let ram_pct = if total_mb > 0 { (used_mb as f32 / total_mb as f32) * 100.0 } else { 0.0 };

        // --- 🏎️ 2. [CPU 영역]: 1초 전 히스토리 기반 비동기 차등 연산 ---
        let (current_idle, current_total) = Self::read_cpu_jiffies().await;
        
        let mut last = LAST_JIFFIES.lock().await;
        let (v1_idle, v1_total) = *last;
        *last = (current_idle, current_total);

        let cpu_pct = if v1_total > 0 {
            let delta_idle = current_idle.saturating_sub(v1_idle);
            let delta_total = current_total.saturating_sub(v1_total);

            if delta_total > 0 {
                (1.0 - (delta_idle as f32 / delta_total as f32)) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        SysMetrics {
            cpu_usage_pct: (cpu_pct * 10.0).round() / 10.0,
            ram_used_mb: used_mb,
            ram_total_mb: total_mb,
            ram_usage_pct: ram_pct,
        }
    }

    /// 📄 [/proc/stat 전용 비밀 요원] 토키오 비동기 파일 래퍼 장착
    async fn read_cpu_jiffies() -> (u64, u64) {
        let stat_info = tokio::fs::read_to_string("/proc/stat").await.unwrap_or_default();
        if let Some(first_line) = stat_info.lines().next() {
            if first_line.starts_with("cpu ") {
                let parts: Vec<u64> = first_line
                    .split_whitespace()
                    .skip(1)
                    .filter_map(|s| u64::from_str(s).ok())
                    .collect();

                if parts.len() >= 4 {
                    let idle = parts[3];
                    let total: u64 = parts.iter().sum();
                    let idle_time = idle + parts.get(4).unwrap_or(&0); // iowait 포함

                    return (idle_time, total);
                }
            }
        }
        (0, 0)
    }
}