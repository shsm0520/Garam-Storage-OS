mod hardware;
mod storage;
mod docker;

use std::fs;
use std::sync::Arc; 
use std::time::{Duration, Instant};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock; 
use tokio::task::LocalSet; 
use serde::Deserialize;
use serde_json::json; 
use log::{ info, warn};

use crate::hardware::boot::BootInspector;
use crate::hardware::power::UpsController;
use crate::hardware::sysmon::SystemMonitor;

// 🔒 [바이너리 프레이밍 코덱 관로]
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec}; 
use tokio_util::bytes::Bytes;
use futures::{StreamExt, SinkExt}; 
use nix::unistd::{chown, Group}; 

#[derive(Deserialize, Debug, Clone)]
struct SystemConfig {
    hostname: String,
    web_port: u16,
    socket_path: String,
    allowed_uids: Vec<u32>,
    enable_auto_mutation: bool, 
    default_cpu_limit: f32,
    default_memory_mb: u64,
    authz_socket_path: String,
    authz_socket_mode: u32,
    authz_group_name: String,
    disk_worker_socket_path: String,
    storage_lock_path: String,
    monitor_target_disk: String,
}

#[derive(Deserialize, Debug, Clone)]
struct IpcRequest {
    cmd: String,
    args: Vec<String>,
}

#[derive(Debug, Clone)]
struct SharedState {
    cpu_usage: f32,
    ram_used_mb: u64,
    ram_total_mb: u64,
    ram_pct: f32,
    disk_temp: String,
    ac_connected: bool,
    battery_pct: u8,
    ups_voltage: f32,
    backup_time_left_min: u32,
    active_disks: Vec<String>,
}

async fn run_daemon(config: SystemConfig) {
    let start_time = Instant::now();
    let version = env!("CARGO_PKG_VERSION");

    // 🔒 시스템 무결성 부팅 검증 레이어
    if let Err(e) = BootInspector::verify_system_integrity() {
        eprintln!("💀 [무결성 붕괴] 시스템 파일 변형 감지: {}", e);
        std::process::exit(1); 
    }

    let initial_disks = hardware::disks::get_fresh_disk_list();

    let shared_state = Arc::new(RwLock::new(SharedState {
        cpu_usage: 0.0,
        ram_used_mb: 0,
        ram_total_mb: 0,
        ram_pct: 0.0,
        disk_temp: "N/A".to_string(),
        ac_connected: true,
        battery_pct: 100,
        ups_voltage: 14.2,
        backup_time_left_min: 180,
        active_disks: initial_disks,
    }));

    // 🔒 [독점 싱글턴 가드 및 투명 프록시 라우터 타설]
    let mut current_socket_path = config.socket_path.clone();
    let mut is_proxy_mode = false;

    if Path::new(&config.socket_path).exists() {
        if UnixStream::connect(&config.socket_path).await.is_ok() {
            warn!("⚠️ [프록시 모드 격발] 가람 데몬 원본(1호기)이 이미 활성화되어 있습니다.");
            info!("🔰 본진 파괴를 막기 위해 2호기 엔진은 '투명 IPC 프록시 수송선'으로 보직 변경합니다.");
            current_socket_path = format!("{}_{}", config.socket_path, uuid::Uuid::new_v4().to_string()[..4].to_string());
            is_proxy_mode = true;
        } else {
            let _ = fs::remove_file(&config.socket_path);
        }
    }

    if !is_proxy_mode {
        info!("==================================================");
        info!(" 가람OS 데몬 엔터프라이즈 네이티브 메인 엔진 시동 완료");
        info!("==================================================");

        // 🔋 [UPS 워치독 태스크]
        let ups_state_share = Arc::clone(&shared_state);
        tokio::task::spawn(async move {
            loop {
                let ups = tokio::task::spawn_blocking(|| {
                    UpsController::read_ups_telemetry()
                })
                .await
                .unwrap_or(hardware::power::UpsStatus {
                    ac_connected: true, battery_pct: 100, ups_voltage: 14.2, backup_time_left_min: 180,
                });
                {
                    let mut lock = ups_state_share.write().await;
                    lock.ac_connected = ups.ac_connected;
                    lock.battery_pct = ups.battery_pct;
                    lock.ups_voltage = ups.ups_voltage;
                    lock.backup_time_left_min = ups.backup_time_left_min;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });

        // 📊 [백그라운드 자원 매립 특공대]
        let state_writer = Arc::clone(&shared_state);
        tokio::spawn(async move {
            loop {
                let metrics = SystemMonitor::fetch_metrics().await;
                let sda_temp = crate::hardware::disks::get_disk_realtime_temperature_only("sda").await;
                {
                    let mut lock = state_writer.write().await;
                    lock.cpu_usage = metrics.cpu_usage_pct;
                    lock.ram_used_mb = metrics.ram_used_mb;
                    lock.ram_total_mb = metrics.ram_total_mb;
                    lock.ram_pct = metrics.ram_usage_pct.round();
                    lock.disk_temp = sda_temp;
                } 
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });

        // 🐳 도커 안보 중개 엔진
        let p_path = config.authz_socket_path.clone();
        let p_mode = config.authz_socket_mode;
        let p_group = config.authz_group_name.clone();
        let p_mut = config.enable_auto_mutation;
        let p_cpu = config.default_cpu_limit;
        let p_mem = config.default_memory_mb;
        tokio::spawn(async move {
            let _ = docker::authz::GaramDockerAuthzEngine::start_checkpoint(&p_path, p_mode, &p_group, p_mut, p_cpu, p_mem).await;
        });
    }

    if Path::new(&current_socket_path).exists() {
        let _ = fs::remove_file(&current_socket_path);
    }
    let listener = UnixListener::bind(&current_socket_path).unwrap();
    let _ = fs::set_permissions(&current_socket_path, fs::Permissions::from_mode(0o770));
    if let Ok(Some(garam_group)) = Group::from_name(&config.authz_group_name) {
        let _ = chown(Path::new(&current_socket_path), None, Some(garam_group.gid));
    }

    info!("📡 가람OS 리스너 가동 채널 개통 완료 -> {}", current_socket_path);
    
    let state_reader = Arc::clone(&shared_state);
    let allowed_uids = config.allowed_uids.clone();
    let original_socket_path = config.socket_path.clone();

    loop {
        if let Ok((stream, _)) = listener.accept().await {
            if let Ok(cred) = stream.peer_cred() {
                if cred.uid() != 0 && !allowed_uids.contains(&cred.uid()) { continue; }
            }

            let state_reader_clone = Arc::clone(&state_reader);
            let hostname = config.hostname.clone();
            let version = version.to_string();
            let orig_path = original_socket_path.clone();

            tokio::spawn(async move {
                let (reader, writer) = tokio::io::split(stream);
                let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());
                let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());

                // 👑 [프록시 인터셉터]: 프록시 모드이면 명령어를 원본 데몬으로 토스
                if is_proxy_mode {
                    if let Ok(main_server_stream) = UnixStream::connect(&orig_path).await {
                        let (m_reader, m_writer) = tokio::io::split(main_server_stream);
                        let mut main_reader = FramedRead::new(m_reader, LengthDelimitedCodec::new());
                        let mut main_writer = FramedWrite::new(m_writer, LengthDelimitedCodec::new());

                        while let Some(Ok(bytes_mut)) = framed_reader.next().await {
                            if main_writer.send(bytes_mut.freeze()).await.is_ok() {
                                if let Some(Ok(response_from_main)) = main_reader.next().await {
                                    let _ = framed_writer.send(response_from_main.freeze()).await;
                                }
                            }
                        }
                    }
                    return;
                }

                // =========================================================================
                // 👑 1호기 메인 엔진 전용 실탄 명령어 연산반
                // =========================================================================
                while let Some(Ok(bytes_mut)) = framed_reader.next().await {
                    let req: IpcRequest = match serde_json::from_str(&String::from_utf8_lossy(&bytes_mut)) {
                        Ok(parsed) => parsed,
                        Err(_) => continue,
                    };

                    match req.cmd.as_str() {
                        "status" => {
                            let is_watch = req.args.contains(&"w".to_string());

                            if is_watch {
                                loop {
                                    let current = state_reader_clone.read().await;
                                    let live_data = json!({
                                        "hostname": hostname,
                                        "uptime_secs": start_time.elapsed().as_secs(),
                                        "version": version,
                                        "hardware": {
                                            "cpu_usage_pct": current.cpu_usage,
                                            "ram_total_mb": current.ram_total_mb,
                                            "ram_used_mb": current.ram_used_mb,
                                            "ram_usage_pct": current.ram_pct,
                                            "disk_temp": current.disk_temp
                                        },
                                        "power_ups": {
                                            "ac_connected": current.ac_connected,
                                            "battery_pct": current.battery_pct,
                                            "voltage": current.ups_voltage,
                                            "runtime_left_min": current.backup_time_left_min
                                        }
                                    });
                                    drop(current); // RwLock 해제 후 sleep

                                    // ✅ [수정] FramedWrite에 그냥 넘기기만 하면 됨
                                    // 수동으로 length prefix 붙이지 않음!
                                    let payload = Bytes::from(live_data.to_string());
                                    if framed_writer.send(payload).await.is_err() { break; }

                                    tokio::time::sleep(Duration::from_millis(500)).await;
                                }
                            } else {
                                // 단발성 요청
                                let current = state_reader_clone.read().await;
                                let live_data = json!({
                                    "hostname": hostname,
                                    "uptime_secs": start_time.elapsed().as_secs(),
                                    "version": version,
                                    "hardware": {
                                        "cpu_usage_pct": current.cpu_usage,
                                        "ram_total_mb": current.ram_total_mb,
                                        "ram_used_mb": current.ram_used_mb,
                                        "ram_usage_pct": current.ram_pct,
                                        "disk_temp": current.disk_temp
                                    },
                                    "power_ups": {
                                        "ac_connected": current.ac_connected,
                                        "battery_pct": current.battery_pct,
                                        "voltage": current.ups_voltage,
                                        "runtime_left_min": current.backup_time_left_min
                                    }
                                });

                                // ✅ [수정] 동일하게 FramedWrite에 그냥 넘기기
                                let payload = Bytes::from(live_data.to_string());
                                let _ = framed_writer.send(payload).await;
                            }
                        }
                        _ => {
                            let _ = framed_writer.send(Bytes::from("🔴 [가람OS] 알 수 없는 명령어입니다")).await;
                        }
                    }
                }
            });
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let config_content = fs::read_to_string("config.toml").unwrap();
    let config: SystemConfig = toml::from_str(&config_content).unwrap();
    let local = LocalSet::new();
    local.run_until(run_daemon(config)).await;
}