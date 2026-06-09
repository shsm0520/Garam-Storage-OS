use super::StorageBackend;
use std::process::Command;
use log::info;

pub struct ZfsBackend;

impl StorageBackend for ZfsBackend {
    fn create_pool(&self, name: &str, raid_type: &str, disks: &[&str]) -> Result<String, String> {
        info!("ZFS 백엔드 가동: pool '{}' 생성 시도 (RAID: {})", name, raid_type);
        let zfs_raid = if raid_type == "raid5" { "raidz" } else { raid_type };
        
        let output = Command::new("zpool")
            .arg("create")
            .arg(name)
            .arg(zfs_raid)
            .args(disks)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                Ok(format!("🏗️ [ZFS 코어] 풀 '{}' 생성 및 커널 마운트 완료!\n", name))
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr);
                Err(format!("❌ [ZFS 커널 거부]: {}\n", err.trim()))
            }
            Err(e) => Err(format!("❌ [시스템 오류] zpool 도구 실행 불가: {}\n", e)),
        }
    }

    fn list_pools(&self) -> Result<String, String> {
        let output = Command::new("zpool").arg("list").output();
        match output {
            Ok(out) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).into_owned()),
            _ => Err("❌ ZFS 풀 목록을 불러올 수 없거나 생성된 풀이 없습니다.\n".to_string()),
        }
    }
}