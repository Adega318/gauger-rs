use std::time::{Duration, Instant};

use bollard::models::ContainerStatsResponse;

/// Live resource usage for a single container.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResourceUsage {
    /// CPU usage as a percentage of one core. `None` until a second sample exists.
    pub cpu_percent: Option<f32>,
    pub memory_used: u64,
    pub memory_limit: u64,
    pub memory_percent: Option<f32>,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub network_rx_rate: Option<u64>,
    pub network_tx_rate: Option<u64>,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
    pub block_read_rate: Option<u64>,
    pub block_write_rate: Option<u64>,
    pub pids: Option<u64>,
    /// When this sample was received. Used to expire stale values.
    pub sampled_at: Instant,
}

/// Cumulative counters kept between samples so rates can be computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatsCounters {
    cpu_total: u64,
    network_rx: u64,
    network_tx: u64,
    block_read: u64,
    block_write: u64,
    sampled_at: Instant,
}

impl ResourceUsage {
    /// Build a usage sample and the counters needed for the next one.
    ///
    /// The first sample (or one following a counter reset) has no rates: only
    /// memory usage is meaningful without a previous point of reference.
    pub fn from_stats(
        stats: &ContainerStatsResponse,
        previous: Option<&StatsCounters>,
        now: Instant,
    ) -> (Self, StatsCounters) {
        let (memory_used, memory_limit) = memory(stats);
        let memory_percent = (memory_limit > 0)
            .then(|| ((memory_used as f64 / memory_limit as f64) * 100.0).min(100.0) as f32);

        let cpu_total = cpu_total(stats);
        let (network_rx, network_tx) = network(stats);
        let (block_read, block_write) = block(stats);

        let counters = StatsCounters {
            cpu_total,
            network_rx,
            network_tx,
            block_read,
            block_write,
            sampled_at: now,
        };

        let Some(previous) = previous else {
            return (
                Self {
                    cpu_percent: None,
                    memory_used,
                    memory_limit,
                    memory_percent,
                    network_rx_bytes: network_rx,
                    network_tx_bytes: network_tx,
                    network_rx_rate: None,
                    network_tx_rate: None,
                    block_read_bytes: block_read,
                    block_write_bytes: block_write,
                    block_read_rate: None,
                    block_write_rate: None,
                    pids: pids(stats),
                    sampled_at: now,
                },
                counters,
            );
        };

        let elapsed = now.saturating_duration_since(previous.sampled_at);

        let cpu_percent = if cpu_total >= previous.cpu_total {
            cpu_percent(cpu_total - previous.cpu_total, elapsed, stats)
        } else {
            None
        };

        (
            Self {
                cpu_percent,
                memory_used,
                memory_limit,
                memory_percent,
                network_rx_bytes: network_rx,
                network_tx_bytes: network_tx,
                network_rx_rate: rate(network_rx, previous.network_rx, elapsed),
                network_tx_rate: rate(network_tx, previous.network_tx, elapsed),
                block_read_bytes: block_read,
                block_write_bytes: block_write,
                block_read_rate: rate(block_read, previous.block_read, elapsed),
                block_write_rate: rate(block_write, previous.block_write, elapsed),
                pids: pids(stats),
                sampled_at: now,
            },
            counters,
        )
    }
}

fn cpu_total(stats: &ContainerStatsResponse) -> u64 {
    stats
        .cpu_stats
        .as_ref()
        .and_then(|cpu| cpu.cpu_usage.as_ref())
        .and_then(|usage| usage.total_usage)
        .unwrap_or(0)
}

fn cpu_percent(delta: u64, elapsed: Duration, stats: &ContainerStatsResponse) -> Option<f32> {
    let elapsed_ns = elapsed.as_nanos();
    if elapsed_ns == 0 {
        return None;
    }

    // Windows reports CPU time in hundreds of nanoseconds; Linux uses nanoseconds.
    let unit_ns = if stats.os_type.as_deref() == Some("windows") {
        100.0
    } else {
        1.0
    };

    let percent = (delta as f64 * unit_ns / elapsed_ns as f64) * 100.0;
    Some(percent as f32)
}

fn memory(stats: &ContainerStatsResponse) -> (u64, u64) {
    let Some(memory) = stats.memory_stats.as_ref() else {
        return (0, 0);
    };

    let raw = memory.usage.or(memory.privateworkingset).unwrap_or(0);

    // Docker excludes inactive page cache from the memory number it reports.
    // cgroups v1 exposes `total_inactive_file`; cgroups v2 exposes `inactive_file`.
    let cache = memory
        .stats
        .as_ref()
        .and_then(|stats| {
            stats
                .get("total_inactive_file")
                .or_else(|| stats.get("inactive_file"))
        })
        .copied()
        .unwrap_or(0);

    let used = raw.saturating_sub(cache.min(raw));

    // Some engines report a sentinel value when no memory limit is configured.
    const UNLIMITED: u64 = 1 << 60;
    let limit = memory.limit.unwrap_or(0);
    (used, if limit >= UNLIMITED { 0 } else { limit })
}

fn network(stats: &ContainerStatsResponse) -> (u64, u64) {
    stats
        .networks
        .as_ref()
        .map(|networks| {
            networks.values().fold((0, 0), |(rx, tx), network| {
                (
                    rx + network.rx_bytes.unwrap_or(0),
                    tx + network.tx_bytes.unwrap_or(0),
                )
            })
        })
        .unwrap_or((0, 0))
}

fn block(stats: &ContainerStatsResponse) -> (u64, u64) {
    let linux = stats
        .blkio_stats
        .as_ref()
        .and_then(|blkio| blkio.io_service_bytes_recursive.as_ref())
        .map(|entries| {
            entries.iter().fold((0, 0), |(read, write), entry| {
                let value = entry.value.unwrap_or(0);

                match entry.op.as_deref() {
                    Some(op) if op.eq_ignore_ascii_case("read") => (read + value, write),
                    Some(op) if op.eq_ignore_ascii_case("write") => (read, write + value),
                    _ => (read, write),
                }
            })
        })
        .unwrap_or((0, 0));

    if linux != (0, 0) || stats.storage_stats.is_none() {
        return linux;
    }

    stats
        .storage_stats
        .as_ref()
        .map(|storage| {
            (
                storage.read_size_bytes.unwrap_or(0),
                storage.write_size_bytes.unwrap_or(0),
            )
        })
        .unwrap_or((0, 0))
}

fn pids(stats: &ContainerStatsResponse) -> Option<u64> {
    stats.pids_stats.as_ref().and_then(|pids| pids.current)
}

fn rate(current: u64, previous: u64, elapsed: Duration) -> Option<u64> {
    if current < previous {
        return None;
    }

    let seconds = elapsed.as_secs_f64();
    if seconds <= 0.0 {
        return None;
    }

    Some(((current - previous) as f64 / seconds).round() as u64)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bollard::models::{
        ContainerBlkioStatEntry, ContainerBlkioStats, ContainerCpuStats, ContainerCpuUsage,
        ContainerMemoryStats, ContainerNetworkStats, ContainerPidsStats, ContainerStatsResponse,
        ContainerStorageStats,
    };

    use super::*;

    fn response() -> ContainerStatsResponse {
        ContainerStatsResponse {
            os_type: Some("linux".to_owned()),
            cpu_stats: Some(ContainerCpuStats {
                cpu_usage: Some(ContainerCpuUsage {
                    total_usage: Some(0),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            memory_stats: Some(ContainerMemoryStats {
                usage: Some(1000),
                limit: Some(2000),
                stats: Some(HashMap::from([("inactive_file".to_owned(), 200)])),
                ..Default::default()
            }),
            networks: Some(HashMap::from([(
                "eth0".to_owned(),
                ContainerNetworkStats {
                    rx_bytes: Some(100),
                    tx_bytes: Some(50),
                    ..Default::default()
                },
            )])),
            blkio_stats: Some(ContainerBlkioStats {
                io_service_bytes_recursive: Some(vec![
                    ContainerBlkioStatEntry {
                        op: Some("read".to_owned()),
                        value: Some(300),
                        ..Default::default()
                    },
                    ContainerBlkioStatEntry {
                        op: Some("write".to_owned()),
                        value: Some(400),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            }),
            pids_stats: Some(ContainerPidsStats {
                current: Some(7),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn first_sample_has_memory_but_no_rates() {
        let now = Instant::now();
        let (usage, _) = ResourceUsage::from_stats(&response(), None, now);

        assert_eq!(usage.cpu_percent, None);
        assert_eq!(usage.memory_used, 800);
        assert_eq!(usage.memory_limit, 2000);
        assert_eq!(usage.memory_percent, Some(40.0));
        assert_eq!(usage.network_rx_bytes, 100);
        assert_eq!(usage.network_tx_bytes, 50);
        assert_eq!(usage.network_rx_rate, None);
        assert_eq!(usage.block_read_bytes, 300);
        assert_eq!(usage.block_write_bytes, 400);
        assert_eq!(usage.pids, Some(7));
        assert_eq!(usage.sampled_at, now);
    }

    #[test]
    fn second_sample_computes_cpu_and_rates() {
        let start = Instant::now();
        let (_, previous) = ResourceUsage::from_stats(&response(), None, start);

        let mut stats = response();
        stats.cpu_stats = Some(ContainerCpuStats {
            cpu_usage: Some(ContainerCpuUsage {
                total_usage: Some(500_000_000),
                ..Default::default()
            }),
            ..Default::default()
        });
        stats
            .networks
            .as_mut()
            .unwrap()
            .get_mut("eth0")
            .unwrap()
            .rx_bytes = Some(1100);
        stats
            .networks
            .as_mut()
            .unwrap()
            .get_mut("eth0")
            .unwrap()
            .tx_bytes = Some(1050);
        stats
            .blkio_stats
            .as_mut()
            .unwrap()
            .io_service_bytes_recursive = Some(vec![
            ContainerBlkioStatEntry {
                op: Some("read".to_owned()),
                value: Some(1300),
                ..Default::default()
            },
            ContainerBlkioStatEntry {
                op: Some("write".to_owned()),
                value: Some(1400),
                ..Default::default()
            },
        ]);

        let now = start + Duration::from_secs(1);
        let (usage, _) = ResourceUsage::from_stats(&stats, Some(&previous), now);

        assert_eq!(usage.cpu_percent, Some(50.0));
        assert_eq!(usage.network_rx_rate, Some(1000));
        assert_eq!(usage.network_tx_rate, Some(1000));
        assert_eq!(usage.block_read_rate, Some(1000));
        assert_eq!(usage.block_write_rate, Some(1000));
    }

    #[test]
    fn counter_reset_clears_rates() {
        let start = Instant::now();

        let mut initial = response();
        initial.cpu_stats = Some(ContainerCpuStats {
            cpu_usage: Some(ContainerCpuUsage {
                total_usage: Some(1_000_000),
                ..Default::default()
            }),
            ..Default::default()
        });
        let (_, previous) = ResourceUsage::from_stats(&initial, None, start);

        let mut stats = response();
        stats.cpu_stats = Some(ContainerCpuStats {
            cpu_usage: Some(ContainerCpuUsage {
                total_usage: Some(10),
                ..Default::default()
            }),
            ..Default::default()
        });
        stats
            .networks
            .as_mut()
            .unwrap()
            .get_mut("eth0")
            .unwrap()
            .rx_bytes = Some(1);
        stats
            .networks
            .as_mut()
            .unwrap()
            .get_mut("eth0")
            .unwrap()
            .tx_bytes = Some(1);

        let now = start + Duration::from_secs(1);
        let (usage, _) = ResourceUsage::from_stats(&stats, Some(&previous), now);

        assert_eq!(usage.cpu_percent, None);
        assert_eq!(usage.network_rx_rate, None);
        assert_eq!(usage.network_tx_rate, None);
    }

    #[test]
    fn missing_memory_limit_hides_percentage() {
        let mut stats = response();
        stats.memory_stats.as_mut().unwrap().limit = Some(0);

        let (usage, _) = ResourceUsage::from_stats(&stats, None, Instant::now());

        assert_eq!(usage.memory_percent, None);
    }

    #[test]
    fn sentinel_memory_limit_is_treated_as_unlimited() {
        let mut stats = response();
        stats.memory_stats.as_mut().unwrap().limit = Some(u64::MAX);

        let (usage, _) = ResourceUsage::from_stats(&stats, None, Instant::now());

        assert_eq!(usage.memory_limit, 0);
        assert_eq!(usage.memory_percent, None);
    }

    #[test]
    fn cgroup_v1_cache_key_is_used() {
        let mut stats = response();
        stats.memory_stats.as_mut().unwrap().stats =
            Some(HashMap::from([("total_inactive_file".to_owned(), 250)]));

        let (usage, _) = ResourceUsage::from_stats(&stats, None, Instant::now());

        assert_eq!(usage.memory_used, 750);
    }

    #[test]
    fn windows_cpu_uses_hundred_nanosecond_units() {
        let mut stats = response();
        stats.os_type = Some("windows".to_owned());

        let start = Instant::now();
        let (_, previous) = ResourceUsage::from_stats(&stats, None, start);

        stats.cpu_stats = Some(ContainerCpuStats {
            cpu_usage: Some(ContainerCpuUsage {
                total_usage: Some(10_000_000),
                ..Default::default()
            }),
            ..Default::default()
        });

        let now = start + Duration::from_secs(1);
        let (usage, _) = ResourceUsage::from_stats(&stats, Some(&previous), now);

        assert_eq!(usage.cpu_percent, Some(100.0));
    }

    #[test]
    fn windows_storage_stats_are_used_without_blkio() {
        let mut stats = response();
        stats.blkio_stats = None;
        stats.storage_stats = Some(ContainerStorageStats {
            read_size_bytes: Some(123),
            write_size_bytes: Some(456),
            ..Default::default()
        });

        let (usage, _) = ResourceUsage::from_stats(&stats, None, Instant::now());

        assert_eq!(usage.block_read_bytes, 123);
        assert_eq!(usage.block_write_bytes, 456);
    }
}
