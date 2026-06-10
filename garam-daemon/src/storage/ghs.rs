//! Garam Hybrid Slicing (GHS) Pure Computation Engine
//! 
//! 사장님의 지시대로 물리 제어를 전면 배제하되, 윗선 집행부가 뇌절하지 않도록
//! 파티션 번호와 섹터 오프셋 족보까지 완벽하게 연산해내는 순수 수학 코어.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use super::StorageBackend; // 👑 부모 트레이트 명확히 수입!

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GhsRaidType {
    Raid1,       // 2대 조각 결속 (골드 존)
    Raid5,       // 3대 이상 조각 결속 (골드 존)
    SafeSingle,  // 홀로 남은 자투리 구출 존 (실버 프리 존 ➔ GFS 2번 패리티 자가치유 버프)
}

/// 🧱 GHS 설계도 조각 내부의 '개별 물리 디스크 파티셔닝' 정밀 지침서
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionTarget {
    pub disk_name: String,       // ex: "sdb"
    pub partition_index: u32,    // ex: 1 -> "sdb1", 2 -> "sdb2"
    pub start_offset_bytes: u64, // ⚡ sfdisk 매핑용 시작 지점 바이트 오프셋
}

/// GHS 연산 엔진이 최종 출력할 '수평 파티션 설계도'의 1개 층 조각 명세서
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhsSlicePlan {
    pub slice_index: u32,
    pub chunk_size_bytes: u64,
    pub raid_type: GhsRaidType,
    pub targets: Vec<PartitionTarget>, // 👑 윗선 집행부가 1초 컷으로 조작할 정밀 명부
}

pub struct PureGhsEngine {
    pub input_disks: Vec<(String, u64)>,
}

impl PureGhsEngine {
    pub fn new(disks: Vec<(String, u64)>) -> Self {
        Self { input_disks: disks }
    }

    /// 🧠 [GHS 심장 수식]: 무한 루프 탈거 후 재귀 가속 파이프라인으로 전격 리팩토링!
    pub fn generate_slicing_blueprint(&self) -> Vec<GhsSlicePlan> {
        let disk_pool: Vec<(String, u64)> = self.input_disks.clone();
        
        // 🛡️ 고정 불멸의 전역 디스크 족보 추적 장부
        let disk_tracker: HashMap<String, (u64, u32)> = self.input_disks
            .iter()
            .map(|(name, _)| (name.clone(), (0u64, 1u32)))
            .collect();

        // 초기 수색대 결과 배열
        let blueprints = Vec::new();

        // 🔄 [재귀 전차 시동]: 1번 슬라이스 층부터 재귀 엔진 구동!
        // 동기 함수 내에서 비동기 스타일 힙 핀을 사용해 동적 호출 스택을 제어합니다.
        let fut = slice_core_recursive(disk_pool, disk_tracker, 1, blueprints);
        
        // 블로킹 방식으로 재귀 최종 결과물 인출 (순수 연산이므로 오버헤드 무부하)
        futures::executor::block_on(fut)
    }
}

/// 🔄 [GHS 전용 핵심 재귀 파이프라인]: 수평 층 분할 정복 기법 완공
fn slice_core_recursive(
    mut disk_pool: Vec<(String, u64)>,
    mut disk_tracker: HashMap<String, (u64, u32)>,
    slice_index: u32,
    mut blueprints: Vec<GhsSlicePlan>,
) -> Pin<Box<dyn Future<Output = Vec<GhsSlicePlan>> + Send>> {
    Box::pin(async move {
        // 1. 🧼 [용량 연소 하드 정제]
        disk_pool.retain(|d| d.1 > 0);

        // 🛑 [기저 조건 / Base Case]: 모든 자투리 용량이 100% 연소되면 즉시 재귀 탈출!
        if disk_pool.is_empty() {
            return blueprints;
        }

        // 2. ⚖️ 남은 용량 기준 정렬 및 가용 최소 chunk 추출
        disk_pool.sort_by_key(|d| d.1);
        let min_chunk_bytes = disk_pool[0].1;
        let available_disks: Vec<String> = disk_pool.iter().map(|d| d.0.clone()).collect();
        let disk_count = available_disks.len();

        // 🚨 [재귀 분기 A]: 홀로 남은 자투리 디스크 독박 처리 명세 발행 및 최종 마감
        if disk_count < 2 {
            let lone_disk_name = &disk_pool[0].0;
            let (offset, part_idx) = disk_tracker.get(lone_disk_name).cloned().unwrap_or((0, 1));

            blueprints.push(GhsSlicePlan {
                slice_index,
                chunk_size_bytes: min_chunk_bytes,
                raid_type: GhsRaidType::SafeSingle,
                targets: vec![PartitionTarget {
                    disk_name: lone_disk_name.clone(),
                    partition_index: part_idx,
                    start_offset_bytes: offset,
                }],
            });
            return blueprints; // 자투리까지 전량 구출 완료로 재귀 종결
        }

        // 3. 👑 황금 레이드 스펙 매칭 
        let raid_type = match disk_count {
            2 => GhsRaidType::Raid1,
            _ => GhsRaidType::Raid5,
        };

        // 4. 이번 층 레이드 파티션 지침서 조립
        let mut targets = Vec::new();
        for d_name in &available_disks {
            let (offset, part_idx) = disk_tracker.get(d_name).cloned().unwrap_or((0, 1));
            
            targets.push(PartitionTarget {
                disk_name: d_name.clone(),
                partition_index: part_idx,
                start_offset_bytes: offset,
            });

            // 🔄 파티션 족보 전진 및 갱신
            if let Some(track) = disk_tracker.get_mut(d_name) {
                track.0 += min_chunk_bytes; 
                track.1 += 1;               
            }
        }

        blueprints.push(GhsSlicePlan {
            slice_index,
            chunk_size_bytes: min_chunk_bytes,
            raid_type,
            targets,
        });

        // 5. 🔪 연산상 두께 차감
        for d in &mut disk_pool {
            d.1 -= min_chunk_bytes;
        }

        // 🔄 [꼬리 재귀 호출 / Tail Call]: 다음 층 명부들을 짊어지고 다음 스택 방으로 수평 진격!
        slice_core_recursive(disk_pool, disk_tracker, slice_index + 1, blueprints).await
    })
}

// =========================================================================
// 👑 [백엔드 연결 문지기] - 순정 100% 동일 유지
// =========================================================================
pub struct GhsBackend;

impl StorageBackend for GhsBackend {
    fn generate_blueprint(&self, _pool_name: &str, disks: &[(String, u64)]) -> Result<Vec<GhsSlicePlan>, String> {
        let pure_engine = PureGhsEngine::new(disks.to_vec());
        let blueprint = pure_engine.generate_slicing_blueprint();
        Ok(blueprint)
    }
}