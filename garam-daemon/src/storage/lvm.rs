use super::StorageBackend;
use crate::storage::ghs::{GhsSlicePlan, GhsRaidType, PartitionTarget}; // 👑 공통 도면 규격 부품 소환
use log::info;

pub struct LvmBackend;

impl StorageBackend for LvmBackend {
    /// 🧠 [LVM 순정 엔진] 복잡한 수평 칼질 없이, 디스크 전체 용량을 통째로 수거하는 단순 명세서 발행
    fn generate_blueprint(&self, _pool_name: &str, disks: &[(String, u64)]) -> Result<Vec<GhsSlicePlan>, String> {
        info!("🏗️ [LVM 백엔드] 순정 리니어 확장 볼륨 도면 연산 개시. 대상: {:?}", disks);
        
        if disks.is_empty() {
            return Err("❌ LVM 구성을 위해서는 최소 1개 이상의 디스크가 필요합니다.\n".to_string());
        }

        let mut blueprints = Vec::new();
        let mut targets = Vec::new();

        // 🧱 LVM의 철학: 하드를 쪼개지 않는다! 들어온 디스크들을 1번 파티션, 오프셋 0 지점부터 통째로 수거한다.
        for (d_name, _size_bytes) in disks {
            targets.push(PartitionTarget {
                disk_name: d_name.clone(),
                partition_index: 1,       // LVM 전용 1번 통짜 파티션
                start_offset_bytes: 0,    // 디스크의 맨 처음(오프셋 0)부터 시작
            });
            
            // 💡 디스크 개별 용량을 그대로 명세에 바인딩
            // 순정 LVM은 각 디스크가 독립적인 크기를 유지한 채 일렬로 결합하므로,
            // 윗선 집행부가 루프를 돌며 각각의 크기대로 파티셔닝할 수 있게 설계도를 뿜어줍니다.
        }

        // LVM은 물리 패리티 보호가 없는 리니어 결합이 기본이므로 
        // 총괄부의 싱글/라이너 빌더(`SafeSingle`) 규격을 차용하여 한 방에 명세를 발행합니다.
        blueprints.push(GhsSlicePlan {
            slice_index: 1, // LVM은 층 분할이 없으므로 언제나 1층 통짜 구조
            chunk_size_bytes: disks.iter().map(|d| d.1).max().unwrap_or(0), // 최대 두께 기준점 토스
            raid_type: GhsRaidType::SafeSingle, // 윗선 집행관이 mdadm linear로 묶도록 유도
            targets,
        });

        // 물리 제어는 윗선(mod.rs) 집행관이 알아서 처리하므로 이 이쁜 통짜 도면만 들고 퇴근!
        Ok(blueprints)
    }
}