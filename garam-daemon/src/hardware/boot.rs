//! Hardware Boot Integrity Checker (Enterprise 14-Core Tools Edition)
use log::{info, warn};
use std::process::{Command, Stdio}; 

pub struct BootInspector;

impl BootInspector {
    /// 🕵️ 가람OS 데몬 구동 전 실전 필수 리눅스 툴 장전 여부 확인
    pub fn verify_system_integrity() -> Result<(), String> {
        info!("🔍 [부팅 검사관] 가람 OS 물리 서브시스템 무결성 검증을 시작합니다.");

        // 👑 [안보 전선 대통합]: 스토리지 파이프라인 및 롤백 엔진이 쓰는 14대 연장을 전수조사합니다!
        let essential_tools = vec![
            "sfdisk",      // 물리 디스크 슬라이스 파티션 타설용
            "mdadm",       // 복합 가상 레이어 복동 결속용 (RAID)
            "pvcreate",    // LVM 물리 볼륨 생성 집행관
            "vgcreate",    // LVM 볼륨 그룹 결속 집행관
            "lvcreate",    // LVM 논리 볼륨 최종 사출관
            "pvremove",    // 트랜잭션 붕괴 시 물리 볼륨 수거책
            "vgremove",    // 트랜잭션 붕괴 시 볼륨 그룹 해체책
            "lvremove",    // 트랜잭션 붕괴 시 논리 볼륨 파괴책
            "mkfs.btrfs",  // GFS 하이엔드 파일시스템 포맷터
            "mount",       // 최상위 스토리지 인프라 실전 마운트 안착 밸브
            "umount",      // 롤백 철수 시 강제 언마운트 해제 밸브
            "findmnt",     // 레이스 컨디션 오염 및 중복 장치 검문관
            "zpool",       // ZFS 가용 제국 풀 빌드 머신
            "dd",          // 롤백 시 파티션 시그니처 제로필 숙청 파괴포
            "blockdev",    // 커널 파티션 테이블 동적 재읽기 관제관
        ];
        let mut missing_tools = Vec::new();

        for tool in essential_tools {
            // 👑 which 경로 인쇄 노이즈 차단막 결속 완료!
            let status = Command::new("which")
                .arg(tool)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
                
            match status {
                Ok(s) if s.success() => {},
                _ => missing_tools.push(tool),
            }
        }

        if !missing_tools.is_empty() {
            warn!("⚠️ [부팅 경고] 현재 커널에 필수 유틸리티가 누락되었습니다: {:?}", missing_tools);
            return Err(format!("❌ 필수 도구 누락으로 부팅 거부: {:?}", missing_tools));
        }

        info!("🟢 [부팅 완공] 14대 커널 바이너리 툴 검증 완벽 패스! 엔진 시동을 허가합니다.");
        Ok(())
    }
}