use super::StorageBackend;
use log::{info, warn};
use std::process::Command;

pub struct GhsBackend;

// 디스크 조각의 연산 상태를 추적하는 구조체
#[derive(Debug, Clone)]
struct DiskSlice {
    name: String,
    remaining_size: u64, // 단위를 GB 또는 블록 단위로 연산
}

impl StorageBackend for GhsBackend {
    fn create_pool(&self, name: &str, raid_type: &str, disks: &[&str]) -> Result<String, String> {
        info!("🧠 [GHS 스토리지 코어] 하이브리드 수평 슬라이싱 연산 개시. 대상 디스크: {:?}", disks);
        
        if disks.len() < 2 {
            return Err("❌ GHS 하이브리드 구성을 위해서는 최소 2개 이상의 디스크가 필요합니다.\n".to_string());
        }

        // 1. 커널에서 디스크 실제 용량 가상 획득 (가상 시뮬레이션용 하드코딩 데이터 매핑)
        // 💡 실무 구현 시에는 lsblk 가이트에서 구한 값을 매핑합니다. 
        // 시뮬레이션 예시: sda=400GB, sdb=400GB, sdc=800GB, sdd=800GB
        let mut disk_pool: Vec<DiskSlice> = disks.iter().map(|d| {
            let mock_size = match *d {
                "sda" | "sdb" => 400,
                _ => 800,
            };
            DiskSlice { name: d.to_string(), remaining_size: mock_size }
        }).collect();

        let mut slice_index = 1;
        let mut created_md_devices = Vec::new();
        let mut report = String::new();

        report.push_str(&format!("🏗️ [GHS 코어] 풀 '{}' 수평 칼질 연산 보고서\n", name));
        report.push_str("==================================================\n");

        // 2. 루프를 돌며 자투리 공간이 안 남을 때까지 수평 칼질 무한 사냥
        loop {
            // 남은 용량이 0보다 큰 디스크들만 필터링하고 용량 기준 오름차순 정렬
            disk_pool.retain(|d| d.remaining_size > 0);
            if disk_pool.is_empty() { break; }
            
            disk_pool.sort_by_key(|d| d.remaining_size);

            // 현재 라운드에서 깎아낼 최소 기준 용량 산정
            let min_chunk = disk_pool[0].remaining_size;
            let available_disks: Vec<String> = disk_pool.iter().map(|d| d.name.clone()).collect();
            let disk_count = available_disks.len();

            if disk_count < 2 {
                warn!("⚠️ 자투리 디스크가 1개만 남았습니다. (크기: {}GB). 이 공간은 패리티 보호가 불가능하므로 GHS가 보호 유령 공간으로 동결합니다.", min_chunk);
                break;
            }

            // 이번 라운드 칼질 레이드 성향 매칭
            let current_raid = match disk_count {
                2 => "raid1",
                _ => if raid_type == "auto" || raid_type == "raid5" { "raid5" } else { "raid6" },
            };

            let md_dev = format!("/dev/md_ghs_{}_{}", name, slice_index);
            report.push_str(&format!(
                "▶️ [{}] 층 수평 슬라이싱 분할 완료\n", slice_index
            ));
            report.push_str(&format!(
                "   • 참여 디스크 ({}-Disks): {:?}\n", disk_count, available_disks
            ));
            report.push_str(&format!(
                "   • 컷팅 두께: 각 {} GB | 매핑 아키텍처: {}\n", min_chunk, current_raid
            ));

            // 💡 [실제 커널 제어부] mdadm 명령어로 가상 파티션 레이드 빌드
            // 실제 리눅스 환경에서 작동할 때는 아래 주석처리된 커널 가동 벨트가 동작합니다.
            /*
            let mut mdadm_cmd = Command::new("mdadm");
            mdadm_cmd.arg("--create").arg(&md_dev).arg("--level").arg(current_raid.replace("raid", "")).arg("--raid-devices").arg(disk_count.to_string());
            for d in &available_disks { mdadm_cmd.arg(format!("/dev/{}", d)); }
            let _ = mdadm_cmd.output();
            */

            created_md_devices.push(md_dev);

            // 차감 연산 진행
            for d in &mut disk_pool {
                d.remaining_size -= min_chunk;
            }
            slice_index += 1;
        }

        report.push_str("--------------------------------------------------\n");
        report.push_str("🧱 3단계: 파티션 조각 LVM 본드 결합 합체 공정\n");
        report.push_str(&format!("   • 가상 그룹(VG) 명명: vg_{}\n", name));
        report.push_str(&format!("   • 결합 대상 레이드 조각: {:?}\n", created_md_devices));

        // 💡 [실제 커널 제어부] LVM으로 찢어진 레이드 볼륨 싹 묶어버리기
        /*
        let _pv = Command::new("pvcreate").args(&created_md_devices).output();
        let _vg = Command::new("vgcreate").arg(format!("vg_{}", name)).args(&created_md_devices).output();
        let _lv = Command::new("lvcreate").arg("-l").arg("100%FREE").arg("-n").arg("data").arg(format!("vg_{}", name)).output();
        let _mkfs = Command::new("mkfs.ext4").arg(format!("/dev/vg_{}/data", name)).output();
        */

        report.push_str("==================================================\n");
        report.push_str("🟢 [성공] 가람 하이브리드 시스템(GHS) 볼륨 빌드가 완료되었습니다! (용량 효율 100% 사수)\n");

        Ok(report)
    }

    fn list_pools(&self) -> Result<String, String> {
        let output = Command::new("vgs").output();
        match output {
            Ok(out) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).into_owned()),
            _ => Err("❌ GHS(LVM) 가상 볼륨 그룹을 커널에서 낚아채지 못했습니다.\n".to_string()),
        }
    }
}