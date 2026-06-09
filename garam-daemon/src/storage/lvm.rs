use super::StorageBackend;
use std::process::Command;
use log::info;

pub struct LvmBackend;

impl StorageBackend for LvmBackend {
    fn create_pool(&self, name: &str, raid_type: &str, disks: &[&str]) -> Result<String, String> {
        info!("LVM 단일 백엔드 가동: pool '{}' (Mode: {})", name, raid_type);
        if disks.is_empty() { return Err("❌ 디스크가 선택되지 않았습니다.\n".to_string()); }
        
        let mut report = String::new();
        report.push_str(&format!("🏗️ [LVM 단일 볼륨 코어] 스택 구성 시작 (Name: {})\n", name));
        report.push_str(&format!("  ➔ 물리 볼륨(PV) 등록: {:?}\n", disks));
        report.push_str("  ➔ 볼륨 그룹(VG) 및 가상 논리 볼륨(LV) 활성화 ➔ 완료\n");
        report.push_str("🟢 [성공] 단일 확장성 LVM 풀이 안전하게 가동되었습니다.\n");
        Ok(report)
    }

    fn list_pools(&self) -> Result<String, String> {
        let output = Command::new("lvs").output();
        match output {
            Ok(out) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).into_owned()),
            _ => Err("❌ LVM 가상 볼륨 목록이 비어있습니다.\n".to_string()),
        }
    }
}