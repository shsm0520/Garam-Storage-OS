//! GaramOS Hardware Management System Subtitle Mod
pub mod disks;   // 💾 디스크 목록 및 SMART 종합 병원
pub mod boot;    // 🛡️ 부팅 무결성 검사관 (추가)
pub mod sysmon;  // 📊 CPU / RAM 자원 감시 요원 (추가)
pub mod power;   // ⚡ UPS 인산철 배터리 통신 총괄 (추가)