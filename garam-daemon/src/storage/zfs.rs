use super::StorageBackend;
use crate::storage::ghs::{GhsSlicePlan, GhsRaidType, PartitionTarget}; // 👑 공통 도면 규격 부품 소환
use log::info;

pub struct ZfsBackend;

impl StorageBackend for ZfsBackend {
    /// 🧠 [ZFS 엔진] 직접 zpool 명령을 날리지 않고, ZFS 가상 vdev 풀 구성을 위한 통짜 도면 명세서 발행
    fn generate_blueprint(&self, _pool_name: &str, disks: &[(String, u64)]) -> Result<Vec<GhsSlicePlan>, String> {
        info!("🏗️ [ZFS 백엔드] 엔터프라이즈 ZFS 풀 구조 도면 연산 개시. 대상: {:?}", disks);
        
        if disks.is_empty() {
            return Err("❌ ZFS 구성을 위해서는 최소 1개 이상의 디스크가 필요합니다.\n".to_string());
        }

        let mut blueprints = Vec::new();
        let mut targets = Vec::new();

        // 🧱 ZFS의 철학: 하드를 수평 칼질하지 않고, 디스크 전체 용량을 1번 파티션 통째로 긁어모은다.
        for (d_name, _size_bytes) in disks {
            targets.push(PartitionTarget {
                disk_name: d_name.clone(),
                partition_index: 1,       // ZFS 전용 통짜 파티션
                start_offset_bytes: 0,    // 오프셋 0 지점부터 통째로 점유
            });
        }

        // 유저가 요청한 레이드 타입 문자열은 윗선 총괄부(mod.rs)에서 최종 매핑하지만,
        // ZFS 백엔드는 디스크 개수를 보고 안전하게 레이드 명세를 번역해서 도면을 뿜어줍니다.
        let raid_type = match disks.len() {
            1 => GhsRaidType::SafeSingle, // 단독 디스크 ZFS 풀
            2 => GhsRaidType::Raid1,      // ZFS Mirroring 구성용 명세
            _ => GhsRaidType::Raid5,      // ZFS RaidZ1 구성용 명세
        };

        // ZFS는 계층 분할이 필요 없는 통짜 가상 풀이므로 1층 명세로 제한하여 리턴
        blueprints.push(GhsSlicePlan {
            slice_index: 1, 
            chunk_size_bytes: disks.iter().map(|d| d.1).max().unwrap_or(0), // 최대 하드 두께 가이드 토스
            raid_type,
            targets,
        });

        // 짚어주신 아키텍처 원칙에 따라, 커널 명령 조작은 싹 빼고 이 고결한 ZFS 도면 명세만 들고 퇴근!
        Ok(blueprints)
    }
}