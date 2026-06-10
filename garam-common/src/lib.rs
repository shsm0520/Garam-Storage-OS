//! GaramOS Internal Common Library & Framework Cores (Enterprise Binary Framing Edition)
use serde::{Serialize, Deserialize};
use tokio::net::UnixStream;
use std::time::Duration;

// 🔒 [안보 결속 완공]: 중복 수입 라인을 완전히 철거하고 퓨어 바이너리 프레임 명부만 장전합니다!
use tokio_util::codec::length_delimited::LengthDelimitedCodec;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::bytes::Bytes; 

use futures::{SinkExt, StreamExt};

// =========================================================================
// 📡 1. 가람OS 전역 인터널 IPC 규격 선로 (바이너리 프레이밍 완전체)
// =========================================================================
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IpcRequest {
    pub cmd: String,
    pub args: Vec<String>,
}

pub async fn send_ipc_request(socket_path: &str, request: &IpcRequest) -> Result<String, String> {
    let stream = UnixStream::connect(socket_path)
        .await
        .map_err(|e| format!("❌ 가람 데몬 본진 소켓 연결 실패: {}", e))?;

    let (reader, writer) = tokio::io::split(stream);
    
    // ⛓️ [4바이트 프레임 레이어 인스턴스화 완료]
    let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());
    let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());

    let payload = serde_json::to_string(request)
        .map_err(|e| format!("❌ 요청 직렬화 실패: {}", e))?;

    // 🚀 4바이트 헤더 프리픽스를 용접하여 소켓 사출
    let _ = framed_writer.send(Bytes::from(payload)).await
        .map_err(|e| format!("❌ 데몬 패킷 송신 실패: {}", e))?;

    // 📡 거대 멀티라인 JSON 패킷 분쇄 유실 없이 원자적 포획
    match framed_reader.next().await {
        Some(Ok(bytes_mut)) => {
            String::from_utf8(bytes_mut.to_vec())
                .map_err(|e| format!("❌ 문자열 복원 찐빠: {}", e))
        }
        Some(Err(e)) => Err(format!("❌ 응답 프레임 디코딩 찐빠: {}", e)),
        None => Err("❌ 데몬이 관로를 폐쇄했습니다.".to_string()),
    }
}

// =========================================================================
// 👑 2. 가람OS 공인 엔터프라이즈 재귀 토큰 파서 엔진
// =========================================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolArgs {
    pub name: String,
    pub engine: String,
    pub raid: String,
    pub disks: Vec<String>,
}

pub fn parse_pool_args_recursive(
    mut args: Vec<String>,
    mut current_state: PoolArgs,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<PoolArgs, String>> + Send>> {
    Box::pin(async move {
        if args.is_empty() {
            if current_state.name.is_empty() {
                return Err("❌ [공통 재귀파서] 스토리지 풀 생성명(`pool_name`)이 빠졌습니다.".to_string());
            }
            if current_state.disks.is_empty() {
                return Err("❌ [공통 재귀파서] 풀에 매립할 물리 디스크 리스트가 전멸했습니다.".to_string());
            }
            return Ok(current_state);
        }

        let head = args.remove(0);

        match head.as_str() {
            "--engine" | "-e" => {
                if args.is_empty() { return Err("❌ [공통 재귀파서] 엔진 타입(-e) 지정 값이 누락되었습니다.".to_string()); }
                current_state.engine = args.remove(0);
            }
            "--raid" | "-r" => {
                if args.is_empty() { return Err("❌ [공통 재귀파서] 레이드 규격(-r) 지정 값이 누락되었습니다.".to_string()); }
                current_state.raid = args.remove(0);
            }
            _ => {
                if current_state.name.is_empty() {
                    current_state.name = head;
                } else if head.starts_with("/dev/") || head.chars().all(|c| c.is_alphanumeric()) {
                    current_state.disks.push(head);
                }
            }
        }

        parse_pool_args_recursive(args, current_state).await
    })
}

// =========================================================================
// 👑 3. 가람 표준 고성능 데이터 포맷터 군단
// =========================================================================
pub fn format_uptime_human_readable(total_secs: u64) -> String {
    let years = total_secs / (365 * 24 * 3600);
    let rem = total_secs % (365 * 24 * 3600);
    
    let months = rem / (30 * 24 * 3600);
    let rem_days = rem % (30 * 24 * 3600);
    
    let days = rem_days / (24 * 3600);
    let rem_hours = rem_days % (24 * 3600);
    
    let hours = rem_hours / 3600;
    let rem_mins = rem_hours % 3600;
    
    let minutes = rem_mins / 60;
    let seconds = rem_mins % 60;

    let mut uptime_string = String::new();
    if years > 0 { uptime_string.push_str(&format!("{}년 ", years)); }
    if months > 0 { uptime_string.push_str(&format!("{}개월 ", months)); }
    if days > 0 { uptime_string.push_str(&format!("{}일 ", days)); }
    uptime_string.push_str(&format!("{}시간 {}분 {}초", hours, minutes, seconds));
    
    uptime_string
}

pub fn make_text_gauge(percentage: usize, width: usize) -> String {
    let filled_count = (percentage * width) / 100;
    let empty_count = width.saturating_sub(filled_count);
    
    "█".repeat(filled_count) + &"░".repeat(empty_count)
}

// =========================================================================
// 🐳 4. 가람OS 격리형 디스크 통제 IPC 프로토콜 명세 & 고속 수송 엔진
// =========================================================================

/// 🚀 [발주서]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiskCommandRequest {
    pub tx_id: String,
    pub command: String,       // "pvcreate", "vgcreate", "mkfs.btrfs", "mount" 등
    pub args: Vec<String>,
    pub timeout_secs: u64,
}

/// 🐳 [영수증]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiskCommandResponse {
    pub tx_id: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// 👑 [전역 소켓 수송 엔진]: 지정된 유닉스 소켓 경로로 디스크 제어 패킷을 쏘고 결과 영수증을 포획합니다.
pub async fn send_disk_worker_command(
    socket_path: &str,
    tx_id: &str,
    command: &str,
    args: &[&str],
    timeout_secs: u64,
) -> Result<DiskCommandResponse, String> {
    
    // 1. 🔗 커널 격리 소켓 통로 노크
    let stream = UnixStream::connect(socket_path)
        .await
        .map_err(|e| format!("❌ [디스크 소켓 통신 실패] 관로({}) 연결 거부: {}", socket_path, e))?;

    let (reader, writer) = tokio::io::split(stream);
    let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());
    let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());

    // 2. 📦 발주서 데이터 구조체 기가 패킹
    let request = DiskCommandRequest {
        tx_id: tx_id.to_string(),
        command: command.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        timeout_secs,
    };

    let payload = serde_json::to_string(&request)
        .map_err(|e| format!("❌ [소켓 엔진] 발주서 직렬화 찐빠: {}", e))?;

    // 3. 🚀 4바이트 프레임 헤더를 붙여서 전격 사출
    let _ = framed_writer.send(Bytes::from(payload)).await
        .map_err(|e| format!("❌ [소켓 엔진] 패킷 송신 실패: {}", e))?;

    // 4. ⏱️ 30초+α 마진 인터록 기반 무한 락업 방어 수령 대기
    let total_timeout = Duration::from_secs(timeout_secs + 2);

    match tokio::time::timeout(total_timeout, framed_reader.next()).await {
        Ok(Some(Ok(bytes_mut))) => {
            let response: DiskCommandResponse = serde_json::from_str(&String::from_utf8_lossy(&bytes_mut))
                .map_err(|e| format!("❌ [소켓 엔진] 영수증 역직렬화 오염: {}", e))?;
            Ok(response)
        }
        Ok(Some(Err(e))) => Err(format!("🚨 [소켓 엔진] 스트림 프레임 디코딩 에러: {}", e)),
        Ok(None) => Err("🚨 [소켓 엔진] 영수증 수령 전에 디스크 에이전트가 관로를 강제 폐쇄했습니다.".to_string()),
        Err(_) => Err(format!("🚨 [소켓 엔진] 디스크 명령 집행 임계 시간({}) 초과로 자동 격리 차단!", timeout_secs)),
    }
}