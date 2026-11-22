//! Host monitoring, mirroring `internal/util/monitor` and
//! `internal/biz/cache/monitor.go`.
//!
//! The snapshot is collected from `/proc` and from the cgroup the process runs
//! in, because a container limited to two cores must not report the host's
//! sixty-four.

use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Serialize)]
pub struct CpuInfo {
    #[serde(rename = "coreCount")]
    pub core_count: i64,
    #[serde(rename = "cpuNum")]
    pub cpu_num: i64,
    pub model: String,
    pub usages: Vec<f64>,
}

#[derive(Serialize)]
pub struct MemoryInfo {
    pub total: u64,
    pub free: u64,
    pub used: u64,
    #[serde(rename = "usedPercent")]
    pub used_percent: f64,
}

#[derive(Serialize)]
pub struct NetInfo {
    #[serde(rename = "bytesSent")]
    pub bytes_sent: u64,
    #[serde(rename = "bytesRecv")]
    pub bytes_recv: u64,
    pub name: String,
}

#[derive(Serialize)]
pub struct DiskInfo {
    pub mountpoint: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    #[serde(rename = "usedPercent")]
    pub used_percent: f64,
}

#[derive(Serialize)]
struct DiskMountpoint {
    device: String,
    mountpoint: String,
    fstype: String,
    opts: Vec<String>,
}

pub fn disk_mountpoints() -> Result<Vec<serde_json::Value>, String> {
    let content = read("/proc/1/mountinfo")
        .or_else(|| read("/proc/self/mountinfo"))
        .ok_or_else(|| "failed to read mount information".to_string())?;
    let mut mounts = Vec::new();
    for line in content.lines() {
        let (left, right) = line
            .split_once(" - ")
            .ok_or_else(|| "invalid mount information".to_string())?;
        let left: Vec<&str> = left.split_whitespace().collect();
        let right: Vec<&str> = right.split_whitespace().collect();
        if left.len() < 6 || right.len() < 2 {
            continue;
        }
        let mut opts: Vec<String> = left[5].split(',').map(str::to_string).collect();
        if left[3] != "/" && !opts.iter().any(|option| option == "bind") {
            opts.push("bind".to_string());
        }
        mounts.push(
            serde_json::to_value(DiskMountpoint {
                device: unescape_mount_field(right[1]),
                mountpoint: unescape_mount_field(left[4]),
                fstype: right[0].to_string(),
                opts,
            })
            .map_err(|error| error.to_string())?,
        );
    }
    Ok(mounts)
}

fn unescape_mount_field(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 3 < bytes.len() {
            let octal = &value[index + 1..index + 4];
            if let Ok(byte) = u8::from_str_radix(octal, 8) {
                output.push(char::from(byte));
                index += 4;
                continue;
            }
        }
        let character = value[index..].chars().next().unwrap_or_default();
        output.push(character);
        index += character.len_utf8();
    }
    output
}

pub fn disk_state(path: &str) -> Result<DiskInfo, String> {
    use std::ffi::CString;

    let path = CString::new(path).map_err(|_| "invalid path".to_string())?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `path` is NUL-terminated and `stats` points to writable storage.
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // SAFETY: statvfs initialized `stats` after returning success.
    let stats = unsafe { stats.assume_init() };
    let block_size = stats.f_bsize as u64;
    let total = (stats.f_blocks as u64).saturating_mul(block_size);
    let free = (stats.f_bavail as u64).saturating_mul(block_size);
    let used = (stats.f_blocks as u64)
        .saturating_sub(stats.f_bfree as u64)
        .saturating_mul(block_size);
    let used_percent = if used + free == 0 {
        0.0
    } else {
        used as f64 / (used + free) as f64 * 100.0
    };
    Ok(DiskInfo {
        mountpoint: path.to_string_lossy().into_owned(),
        total,
        used,
        free,
        used_percent,
    })
}

/// The Go collector's map keys, refreshed in the background.
///
/// `internal/biz/cache/monitor.go` runs one collection immediately at start and
/// then every `cacheSecond` (3) seconds, and `GetSnapshot` only copies the last
/// result. Requesting a snapshot therefore never waits for a CPU sample. This
/// cache mirrors that: `start()` collects once and spawns a three-second ticker.
static SNAPSHOT: std::sync::OnceLock<tokio::sync::RwLock<BTreeMap<String, serde_json::Value>>> =
    std::sync::OnceLock::new();

fn cache() -> &'static tokio::sync::RwLock<BTreeMap<String, serde_json::Value>> {
    SNAPSHOT.get_or_init(|| tokio::sync::RwLock::new(BTreeMap::new()))
}

/// Collect once, then keep the cache fresh. Mirrors `Monitor.Start`: the first
/// collection is complete before the caller returns, so the first request after
/// startup already sees data, exactly as the Go service does.
pub async fn start() {
    collect().await;
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(3));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // The interval's first tick is immediate; the initial collect above
        // already did that work, so wait for the next one.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            collect().await;
        }
    });
}

/// One collection. A piece that fails keeps the previous value, as the Go
/// collector does by only inserting the values that did not error.
async fn collect() {
    let mut values = BTreeMap::new();
    if let Ok(cpu) = cpu_info().await {
        values.insert(
            "CPU_INFO".to_string(),
            serde_json::to_value(cpu).unwrap_or_default(),
        );
    }
    if let Ok(memory) = memory_info() {
        values.insert(
            "MEMORY_INFO".to_string(),
            serde_json::to_value(memory).unwrap_or_default(),
        );
    }
    values.insert(
        "NETWORK_INFO".to_string(),
        serde_json::to_value(net_info()).unwrap_or_default(),
    );
    let mut guard = cache().write().await;
    for (key, value) in values {
        guard.insert(key, value);
    }
}

/// The last collected snapshot. Empty until the first collection finishes,
/// which `start()` completes before the listener opens.
pub async fn snapshot() -> BTreeMap<String, serde_json::Value> {
    cache().read().await.clone()
}

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// One `/proc/stat` line per logical core: (idle, total) jiffies.
fn cpu_times() -> Vec<(u64, u64)> {
    let Some(content) = read("/proc/stat") else {
        return Vec::new();
    };
    let mut cores = Vec::new();
    for line in content.lines() {
        if !line.starts_with("cpu") || line.starts_with("cpu ") {
            continue;
        }
        let mut fields = line.split_whitespace();
        fields.next();
        let values: Vec<u64> = fields.filter_map(|value| value.parse().ok()).collect();
        if values.len() < 5 {
            continue;
        }
        // user, nice, system, idle, iowait, irq, softirq, steal, ...
        let idle = values[3] + values[4];
        let total: u64 = values.iter().sum();
        cores.push((idle, total));
    }
    cores
}

fn cpu_model() -> String {
    let Some(content) = read("/proc/cpuinfo") else {
        return String::new();
    };
    for line in content.lines() {
        if let Some((key, value)) = line.split_once(':') {
            if key.trim() == "model name" {
                return value.trim().to_string();
            }
        }
    }
    String::new()
}

/// `parseCPUSetCount`: "0-3,6" → 5.
fn cpuset_count(content: &str) -> Option<i64> {
    let mut total = 0i64;
    for part in content.trim().split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.split_once('-') {
            Some((start, end)) => {
                let start: i64 = start.trim().parse().ok()?;
                let end: i64 = end.trim().parse().ok()?;
                if end < start {
                    return None;
                }
                total += end - start + 1;
            }
            None => {
                part.parse::<i64>().ok()?;
                total += 1;
            }
        }
    }
    (total > 0).then_some(total)
}

fn first_file(dirs: &[&str], name: &str) -> Option<String> {
    dirs.iter().find_map(|dir| read(&format!("{dir}/{name}")))
}

fn cgroup_dirs(kind: &str) -> Vec<String> {
    // cgroup v2 exposes every controller on one tree; v1 splits them, and the
    // process's own slice is under /proc/self/cgroup.
    let mut dirs = vec!["/sys/fs/cgroup".to_string()];
    if let Some(content) = read("/proc/self/cgroup") {
        for line in content.lines() {
            let Some((_, path)) = line.split_once("::") else {
                continue;
            };
            if !path.trim().is_empty() && path.trim() != "/" {
                dirs.push(format!("/sys/fs/cgroup{}", path.trim()));
            }
        }
        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() == 3 && parts[1].contains(kind) {
                dirs.push(format!("/sys/fs/cgroup/{}{}", kind, parts[2]));
            }
        }
    }
    dirs
}

fn quota_cores() -> Option<f64> {
    let dirs: Vec<String> = cgroup_dirs("cpu");
    let refs: Vec<&str> = dirs.iter().map(String::as_str).collect();
    if let Some(content) = first_file(&refs, "cpu.max") {
        let mut parts = content.split_whitespace();
        let quota = parts.next()?;
        if quota == "max" {
            return None;
        }
        let quota: f64 = quota.parse().ok()?;
        let period: f64 = parts.next()?.parse().ok()?;
        if period <= 0.0 {
            return None;
        }
        return Some(quota / period);
    }
    let quota: f64 = first_file(&refs, "cpu.cfs_quota_us")?.trim().parse().ok()?;
    let period: f64 = first_file(&refs, "cpu.cfs_period_us")?
        .trim()
        .parse()
        .ok()?;
    if quota <= 0.0 || period <= 0.0 {
        return None;
    }
    Some(quota / period)
}

fn cpuset_cores() -> Option<i64> {
    let dirs: Vec<String> = cgroup_dirs("cpuset");
    let refs: Vec<&str> = dirs.iter().map(String::as_str).collect();
    for name in ["cpuset.cpus.effective", "cpuset.cpus"] {
        if let Some(content) = first_file(&refs, name) {
            if let Some(count) = cpuset_count(&content) {
                return Some(count);
            }
        }
    }
    None
}

/// `effectiveCores`.
fn effective_cores(host: i64, quota: Option<f64>, cpuset: Option<i64>) -> i64 {
    let host = host.max(1);
    let mut effective = host;
    if let Some(quota) = quota {
        effective = quota.ceil() as i64;
        if let Some(cpuset) = cpuset {
            effective = effective.min(cpuset);
        }
    } else if let Some(cpuset) = cpuset {
        effective = cpuset;
    }
    effective.clamp(1, host)
}

async fn cpu_info() -> Result<CpuInfo, String> {
    let before = cpu_times();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let after = cpu_times();
    let mut usages = Vec::new();
    for (index, (idle_before, total_before)) in before.iter().enumerate() {
        let Some((idle_after, total_after)) = after.get(index) else {
            continue;
        };
        let total = total_after.saturating_sub(*total_before) as f64;
        let idle = idle_after.saturating_sub(*idle_before) as f64;
        if total <= 0.0 {
            usages.push(0.0);
            continue;
        }
        usages.push(((total - idle) / total * 100.0).clamp(0.0, 100.0));
    }
    let host = if usages.is_empty() {
        1
    } else {
        usages.len() as i64
    };
    let effective = effective_cores(host, quota_cores(), cpuset_cores());
    let sum: f64 = usages.iter().sum();
    let percent = (sum / effective as f64).clamp(0.0, 100.0);
    Ok(CpuInfo {
        core_count: effective,
        cpu_num: effective,
        model: cpu_model(),
        usages: vec![percent],
    })
}

/// `/proc/meminfo` in bytes.
fn meminfo() -> BTreeMap<String, u64> {
    let mut values = BTreeMap::new();
    let Some(content) = read("/proc/meminfo") else {
        return values;
    };
    for line in content.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let mut parts = rest.split_whitespace();
        let Some(value) = parts.next().and_then(|value| value.parse::<u64>().ok()) else {
            continue;
        };
        let multiplier = match parts.next() {
            Some("kB") => 1024,
            _ => 1,
        };
        values.insert(key.trim().to_string(), value * multiplier);
    }
    values
}

fn cgroup_memory() -> Option<(u64, u64, u64)> {
    let dirs: Vec<String> = cgroup_dirs("");
    let refs: Vec<&str> = dirs.iter().map(String::as_str).collect();
    let limit = first_file(&refs, "memory.max")
        .or_else(|| first_file(&refs, "memory.limit_in_bytes"))
        .and_then(|content| {
            let content = content.trim();
            if content == "max" {
                return None;
            }
            content.parse::<u64>().ok()
        })?;
    let usage = first_file(&refs, "memory.current")
        .or_else(|| first_file(&refs, "memory.usage_in_bytes"))
        .and_then(|content| content.trim().parse::<u64>().ok())?;
    let inactive = first_file(&refs, "memory.stat")
        .and_then(|content| {
            content.lines().find_map(|line| {
                let (key, value) = line.split_once(' ')?;
                if key == "inactive_file" || key == "total_inactive_file" {
                    value.trim().parse::<u64>().ok()
                } else {
                    None
                }
            })
        })
        .unwrap_or(0);
    Some((limit, usage, inactive))
}

fn memory_info() -> Result<MemoryInfo, String> {
    let values = meminfo();
    let total = *values.get("MemTotal").unwrap_or(&0);
    if total == 0 {
        return Err("no memory information".to_string());
    }
    if let Some((limit, usage, inactive)) = cgroup_memory() {
        if limit > 0 && limit < total {
            let mut used = usage.saturating_sub(inactive);
            if used > limit {
                used = limit;
            }
            return Ok(MemoryInfo {
                total: limit,
                free: limit - used,
                used,
                used_percent: used as f64 / limit as f64 * 100.0,
            });
        }
    }
    let free = *values.get("MemFree").unwrap_or(&0);
    let buffers = *values.get("Buffers").unwrap_or(&0);
    // gopsutil folds reclaimable slab into the cache before computing `Used`,
    // and does not subtract shared memory.
    let cached = *values.get("Cached").unwrap_or(&0) + *values.get("SReclaimable").unwrap_or(&0);
    let used = total
        .saturating_sub(free)
        .saturating_sub(buffers)
        .saturating_sub(cached);
    Ok(MemoryInfo {
        total,
        free,
        used,
        used_percent: used as f64 / total as f64 * 100.0,
    })
}

fn net_info() -> Vec<NetInfo> {
    let Some(content) = read("/proc/net/dev") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in content.lines().skip(2) {
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if fields.len() < 9 {
            continue;
        }
        let bytes_recv: u64 = fields[0].parse().unwrap_or(0);
        let bytes_sent: u64 = fields[8].parse().unwrap_or(0);
        out.push(NetInfo {
            bytes_sent,
            bytes_recv,
            name: name.trim().to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{disk_state, unescape_mount_field};

    #[test]
    fn mount_fields_decode_kernel_octal_escapes() {
        assert_eq!(
            unescape_mount_field("/mnt/with\\040space"),
            "/mnt/with space"
        );
        assert_eq!(unescape_mount_field("/path\\134name"), "/path\\name");
    }

    #[test]
    fn disk_state_returns_usage_for_existing_path() {
        let state = disk_state("/").expect("root filesystem stats");
        assert_eq!(state.mountpoint, "/");
        assert!(state.total > 0);
        assert!(state.used_percent.is_finite());
    }

    #[test]
    fn disk_state_rejects_missing_path() {
        assert!(disk_state("/yin-panel-no-such-mountpoint").is_err());
    }
}
