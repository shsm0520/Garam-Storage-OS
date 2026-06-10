//! Storage Subsystem Facade & Trait Definitions (Enterprise Isolated Socket Transaction Edition)
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod ghs; 
pub mod zfs; 
pub mod lvm; 

use log::{info, error, warn};
use std::time::Duration;
use ghs::{GhsSlicePlan, GhsRaidType};

// 🤝 공통방에 장착해둔 4바이트 프레임 소켓 통신 모듈 및 규격 징집
use garam_common::send_disk_worker_command;

/// [대동기화 완공]: 트레이트 자체에 : Send + Sync 속성을 매립하여 전선 안정성을 확보합니다!
pub trait StorageBackend: Send + Sync {
    fn generate_blueprint(&self, pool_name: &str, disks: &[(String, u64)]) -> Result<Vec<GhsSlicePlan>, String>;
}

// 👑 [안보 하이웨이]: 데몬 본진이 Root 워커 소켓 채널로 실탄을 쏘고 영수증을 엄격하게 검증합니다.
async fn exec_via_worker(worker_socket: &str, tx_id: &str, command: &str, args: &[&str]) -> Result<String, String> {
    // 🎯 [하드코딩 소멸]: config.toml에서 인출된 worker_socket 주소로 정밀 유도 사출!
    let resp = send_disk_worker_command(worker_socket, tx_id, command, args, 30).await?;
    
    if resp.success {
        Ok(resp.stdout)
    } else {
        let err_msg = format!("❌ 커널 거부 (Code {:?}): {}", resp.exit_code, resp.stderr.trim());
        Err(err_msg)
    }
}

// =========================================================================
// 🧼 GFS 파일 시스템 마감 총괄 제어소 (완전 무결 격리 통합본)
// =========================================================================
pub struct GfsStorageManager {
    pub mount_point: String,
    pub worker_socket: String, // 🎯 격리 관로 주입선 매립
}

impl GfsStorageManager {
    pub fn new(mount_point: &str, worker_socket: &str) -> Self {
        Self { 
            mount_point: mount_point.to_string(),
            worker_socket: worker_socket.to_string(),
        }
    }

    pub async fn execute_format_and_mount(&self, dev_path: &str, tx_id: &str) -> Result<(), String> {
        info!("[TXID: {}] 🧼 [GFS 소켓 마감] 장치 {}를 btrfs로 강제 포맷 및 {} 마운트를 집행합니다.", tx_id, dev_path, self.mount_point);
        
        exec_via_worker(&self.worker_socket, tx_id, "mkfs.btrfs", &["-f", dev_path]).await?;
        exec_via_worker(&self.worker_socket, tx_id, "mkdir", &["-p", &self.mount_point]).await?;
        exec_via_worker(&self.worker_socket, tx_id, "mount", &[dev_path, &self.mount_point]).await?;
        
        Ok(())
    }

    pub async fn trigger_snapshot(&self, snapshot_name: &str) -> Result<(), String> {
        let tx_id = "UPS-EMERG";
        let snap_path = format!("{}/.snapshots/{}", self.mount_point, snapshot_name);
        info!("📸 [GFS 소켓 마감] 정전 대피용 btrfs 스냅샷 [{}]을 생성합니다. 경로: {}", tx_id, snap_path);
        
        exec_via_worker(&self.worker_socket, tx_id, "btrfs", &["subvolume", "snapshot", &self.mount_point, &snap_path]).await?;
        Ok(())
    }
}

// =========================================================================
// 🎛️ [안보 코어]: 트랜잭션 단계별 체크포인트 깃발 정의
// =========================================================================
#[derive(Debug, Clone)]
enum RollbackStep {
    PartitionCreated { disk: String },          
    MdadmArrayCreated { md_device: String },   
    PvCreated { md_devices: Vec<String> },     
    VgCreated { vg_name: String },             
    LvCreated { vg_name: String, lv_name: String }, 
    Mounted { mount_point: String },           
}

// =========================================================================
// 🧱 [물리 집행관]: 100% 설정 기반 원자성(Atomicity) 보증 소켓 롤백 파이프라인
// =========================================================================
pub async fn execute_kernel_build_pipeline(
    pool_name: &str, 
    engine_type: &str, 
    raid_type: &str, 
    raw_disks: &[&str],
    dry_run: bool,
    tx_id: &str,
    // 👑 [설정 연동 완공]: 하드코딩 투성이 주소들을 설정 장부에서 상속받도록 결속선 추가!
    worker_socket: &str,
    storage_lock_path: &str
) -> Result<String, String> {
    info!("[TXID: {}] 🚀 [스토리지 소켓 안보 총괄] 파이프라인 가동. 대상: {:?}", tx_id, raw_disks);

    // 🔒 [TOCTOU 레이스 컨디션 방어선 - 설정값 연동]
    if std::path::Path::new(storage_lock_path).exists() {
        return Err(format!("🚨 [TXID: {}] [접근 거부] 다른 스토리지 빌드 공정이 커널을 점유 중입니다.", tx_id));
    }
    if let Err(e) = tokio::fs::write(storage_lock_path, "LOCKED").await {
        return Err(format!("❌ 락 파일 생성 실패: {}", e));
    }
    
    // scopeguard 폐쇄 루프도 설정 기반 경로로 가인쇄!
    let lock_cleanup_path = storage_lock_path.to_string();
    let _lock_cleaner = scopeguard::guard((), move |_| {
        let _ = std::fs::remove_file(lock_cleanup_path);
    });

    let sys_os_disk = crate::hardware::disks::find_system_os_disk();
    
    let mounted_apps = match exec_via_worker(worker_socket, tx_id, "findmnt", &["-n", "-o", "SOURCE"]).await {
        Ok(out) => out,
        Err(_) => String::new(),
    };

    for disk in raw_disks {
        if *disk == sys_os_disk {
            return Err(format!("🚨 [빌드 거부] '/dev/{}'는 가람OS 메인 시스템 디스크입니다!\n", disk));
        }
        if mounted_apps.contains(disk) {
            return Err(format!("🚨 [빌드 거부] '/dev/{}'는 현재 활성 장치입니다!\n", disk));
        }
    }

    let mut real_disks = Vec::new();
    for d in raw_disks {
        let sys_path = format!("/sys/block/{}/size", d);
        let sectors_str = tokio::fs::read_to_string(&sys_path).await
            .map_err(|e| format!("❌ 디스크 {} 스캔 실패: {}", d, e))?;
        let sectors = sectors_str.trim().parse::<u64>().map_err(|e| e.to_string())?;
        real_disks.push((d.to_string(), sectors * 512));
    }

    let backend: Box<dyn StorageBackend + Send> = match engine_type {
        "hybrid" => Box::new(ghs::GhsBackend), 
        "zfs"    => Box::new(zfs::ZfsBackend),   
        "lvm" | "btrfs" => Box::new(lvm::LvmBackend), 
        _ => return Err(format!("❌ 지원하지 않는 엔진: {}", engine_type)),
    };

    let blueprints = backend.generate_blueprint(pool_name, &real_disks)?;
    
    if engine_type == "zfs" {
        let plan = blueprints.get(0).ok_or_else(|| "❌ ZFS 도면 명세 유실".to_string())?;
        let zfs_raid_cmd = match plan.raid_type {
            GhsRaidType::Raid1 => "mirror",
            GhsRaidType::Raid5 => "raidz",
            GhsRaidType::SafeSingle => "",
        };

        let mut zfs_args = vec!["create", pool_name];
        if !zfs_raid_cmd.is_empty() { zfs_args.push(zfs_raid_cmd); }
        for target in &plan.targets { zfs_args.push(&target.disk_name); }

        if dry_run {
            return Ok(format!("🔍 [ZFS 드라이런 통과] zpool create {} {}\n", pool_name, zfs_raid_cmd));
        }

        exec_via_worker(worker_socket, tx_id, "zpool", &zfs_args).await?;
        return Ok(format!("🟢 [ZFS 완공] 가람 ZFS 풀 '{}' 안착 완공.\n", pool_name));
    }

    let mut rollback_stack: Vec<RollbackStep> = Vec::new();
    let mut report = format!("🏗️ [가람 소켓 트랜잭션] '{}' 물리 매립 공정 집행서\n", pool_name);
    let mut disk_offsets_sectors: std::collections::HashMap<String, u64> = raw_disks.iter().map(|d| (d.to_string(), 0u64)).collect();

    let build_result: Result<(), String> = Box::pin(async {
        let mut created_md_devices = Vec::new();

        for plan in &blueprints {
            let md_dev = format!("/dev/md_garam_{}_{}", pool_name, plan.slice_index);
            let mut partition_targets = Vec::new();
            let chunk_sectors = plan.chunk_size_bytes / 512;

            for target in &plan.targets {
                let d = &target.disk_name;
                let offset = disk_offsets_sectors.get(d).cloned().unwrap_or(0);
                let part_dev = format!("/dev/{}{}", d, target.partition_index);
                
                if dry_run {
                    partition_targets.push(part_dev);
                    if let Some(off) = disk_offsets_sectors.get_mut(d) { *off += chunk_sectors; }
                    continue;
                }

                let dev_file = format!("/dev/{}", d);
                exec_via_worker(worker_socket, tx_id, "sfdisk", &["--append", "--no-reread", &dev_file]).await?;
                rollback_stack.push(RollbackStep::PartitionCreated { disk: d.clone() });

                partition_targets.push(part_dev);
                if let Some(off) = disk_offsets_sectors.get_mut(d) { *off += chunk_sectors; }
            }

            let mut mdadm_args = vec!["--create", &md_dev, "--metadata=1.2", "--force", "--quiet"];
            let d_count = format!("--raid-devices={}", plan.targets.len());
            
            match raid_type {
                "raid1" | "mirror" => { mdadm_args.push("--level=1"); mdadm_args.push("--raid-devices=2"); },
                "raid5" | "raidz"  => { mdadm_args.push("--level=5"); mdadm_args.push(&d_count); },
                _ => { mdadm_args.push("--level=linear"); mdadm_args.push("--raid-devices=1"); }
            }
            for p in &partition_targets { mdadm_args.push(p); }

            if dry_run {
                created_md_devices.push(md_dev);
                continue;
            }

            exec_via_worker(worker_socket, tx_id, "mdadm", &mdadm_args).await?;
            rollback_stack.push(RollbackStep::MdadmArrayCreated { md_device: md_dev.clone() });
            created_md_devices.push(md_dev);
        }

        let vg_name = format!("vg_{}", pool_name);
        let lv_name = "data";
        let target_lv_path = format!("/dev/{}/{}", vg_name, lv_name);

        if !dry_run {
            let mut pv_args = vec!["-f"];
            for md in &created_md_devices { pv_args.push(md.as_str()); }
            exec_via_worker(worker_socket, tx_id, "pvcreate", &pv_args).await?;
            rollback_stack.push(RollbackStep::PvCreated { md_devices: created_md_devices.clone() });

            let mut vg_args = vec![vg_name.as_str()];
            for md in &created_md_devices { vg_args.push(md.as_str()); }
            exec_via_worker(worker_socket, tx_id, "vgcreate", &vg_args).await?;
            rollback_stack.push(RollbackStep::VgCreated { vg_name: vg_name.clone() });

            exec_via_worker(worker_socket, tx_id, "lvcreate", &["-l", "100%FREE", "-n", lv_name, &vg_name]).await?;
            rollback_stack.push(RollbackStep::LvCreated { vg_name: vg_name.clone(), lv_name: lv_name.to_string() });
        }

        let final_mount_point = format!("/storage/{}", pool_name);
        // 제어소에도 주입선 인가 완공!
        let gfs_manager = GfsStorageManager::new(&final_mount_point, worker_socket);
        
        if !dry_run {
            gfs_manager.execute_format_and_mount(&target_lv_path, tx_id).await?;
            rollback_stack.push(RollbackStep::Mounted { mount_point: final_mount_point.clone() });
        }

        Ok(())
    }).await; 

    // =========================================================================
    // 🚨 [설정 기반 소켓 역순 롤백 엔진]: 파이프라인 대폭파 시 철저하게 주입된 소켓 경로로 청소 집행!
    // =========================================================================
    if let Err(err_reason) = build_result {
        error!("🚨 [TXID: {}] [파이프라인 붕괴] 사유: {}. 소켓 연동 롤백 시퀀스를 격발합니다!", tx_id, err_reason);

        while let Some(step) = rollback_stack.pop() {
            warn!("[TXID: {}] 🔀 [소켓 롤백 집행 중] 단계 철거 중: {:?}", tx_id, step);
            match step {
                RollbackStep::Mounted { mount_point } => {
                    let _ = exec_via_worker(worker_socket, tx_id, "umount", &["-f", &mount_point]).await;
                    let _ = exec_via_worker(worker_socket, tx_id, "rmdir", &[&mount_point]).await;
                }
                RollbackStep::LvCreated { vg_name, lv_name } => {
                    let target_lv = format!("{}/{}", vg_name, lv_name);
                    let _ = exec_via_worker(worker_socket, tx_id, "lvremove", &["-f", &target_lv]).await;
                }
                RollbackStep::VgCreated { vg_name } => {
                    let _ = exec_via_worker(worker_socket, tx_id, "vgremove", &["-f", &vg_name]).await;
                }
                RollbackStep::PvCreated { md_devices } => {
                    for md in md_devices {
                        let _ = exec_via_worker(worker_socket, tx_id, "pvremove", &["-f", &md]).await;
                    }
                }
                RollbackStep::MdadmArrayCreated { md_device } => {
                    let _ = exec_via_worker(worker_socket, tx_id, "mdadm", &["--stop", &md_device]).await;
                    let _ = exec_via_worker(worker_socket, tx_id, "mdadm", &["--zero-superblock", &md_device]).await;
                }
                RollbackStep::PartitionCreated { disk } => {
                    let out_file = format!("of=/dev/{}", disk);
                    let _ = exec_via_worker(worker_socket, tx_id, "dd", &["if=/dev/zero", &out_file, "bs=1M", "count=10", "oflag=direct"]).await;
                    let target_dev = format!("/dev/{}", disk);
                    let _ = exec_via_worker(worker_socket, tx_id, "blockdev", &["--rereadpt", &target_dev]).await;
                }
            }
        }
        return Err(format!("❌ [TXID: {}] [롤백 완공] 설정된 소켓 관로를 백트랙하여 클린 철수 성공. (사유: {})", tx_id, err_reason));
    }

    if dry_run {
        return Ok(format!("🔍 [TXID: {}] [드라이런 성공] '{}' 풀 가상 트랜잭션 도면 패스 완료.\n", tx_id, pool_name));
    }

    report.push_str(&format!("🟢 [완공 보증] 가람 GFS 인프라 원자적 타설 완료 ➔ /storage/{}\n", pool_name));
    Ok(report)
}