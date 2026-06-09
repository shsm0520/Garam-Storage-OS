mod hardware;
mod storage;

use std::fs;
use std::time::{Duration, Instant};
use tokio::net::UnixListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde::Deserialize;
use log::{info, warn, error};
use udev::MonitorBuilder;
use tokio::task::LocalSet;

// 동생 방들에서 공용(pub) 부품들 명확히 소환!
use storage::{StorageBackend, zfs::ZfsBackend, ghs::GhsBackend, lvm::LvmBackend};

#[derive(Deserialize, Debug)]
struct SystemConfig {
    hostname: String,
    web_port: u16,
    socket_path: String,
}

#[derive(Deserialize, Debug)]
struct IpcRequest {
    cmd: String,
    args: Vec<String>,
}

async fn run_daemon(config: SystemConfig) {
    let start_time = Instant::now();
    let version = env!("CARGO_PKG_VERSION");

    info!("==================================================");
    info!(" 가람OS 데몬 엔터프라이즈 네이티브 엔진 시동 완료");
    info!("   • 호스트명       : {}", config.hostname);
    info!("   • 웹 서비스 포트 : {}", config.web_port);
    info!("   • 통신 소켓 채널 : Unix Socket ({})", config.socket_path);
    info!("==================================================");

    // 🔋 [신설 - 전원 안전조치 비동기 워치독] 메인 리스너와 별개로 5초마다 감시
    tokio::task::spawn_local(async {
        loop {
            let power = hardware::power::fetch_power_status();
            if !power.ac_online {
                error!("🚨 [가람 위기 관리] 시스템 정전 발생!! UPS 배터리로 긴급 구동 중 (잔량: {}%)", power.battery_pct);
                if power.battery_pct < 20 {
                    error!("💀 배터리 임계치 위험선 돌파! 스토리지 커널 보호를 위해 세이프 셧다운을 선포합니다.");
                    // std::process::Command::new("shutdown").arg("-h").arg("now").output();
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    // 1) udev 감시 허브 백그라운드 구동 (Local Thread)
    tokio::task::spawn_local(async {
        let monitor = MonitorBuilder::new()
            .expect("❌ udev 모니터 생성 실패")
            .match_subsystem("block")
            .expect("❌ udev 필터 매칭 실패")
            .listen()
            .expect("❌ udev 소켓 바인딩 실패");

        loop {
            if let Some(event) = monitor.iter().next() {
                let dev_node = event.devnode().unwrap_or(std::path::Path::new("unknown"));
                let action_type = event.action().unwrap_or(std::ffi::OsStr::new("unknown"));
                
                let dev_path = dev_node.to_string_lossy();
                let action = action_type.to_string_lossy();

                if dev_path.contains("loop") || dev_path.contains("sr") { continue; }

                match action.as_ref() {
                    "add" => warn!("🔥 [하드웨어 인입 감지] 새로운 스토리지 장장 장착! 👉 장치경로: {}", dev_path),
                    "remove" => error!("🔴 [하드웨어 위기 감지] 스토리지가 물리적으로 탈거됨! 👉 장치경로: {}", dev_path),
                    _ => {}
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    let listener = UnixListener::bind(&config.socket_path).unwrap();
    info!("가람OS 데몬 메인 소켓 리스너 대기 중...");
    
    loop {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buffer = [0; 4096]; // 0 초기화 완료 버퍼
            if let Ok(n) = stream.read(&mut buffer).await {
                if n == 0 { continue; }
                let raw_json = String::from_utf8_lossy(&buffer[..n]);
                
                let req: IpcRequest = match serde_json::from_str::<IpcRequest>(&raw_json) {
                    Ok(parsed) => {
                        info!("CMD 수신: '{}'", parsed.cmd);
                        parsed
                    },
                    Err(_) => {
                        let _ = stream.write_all("❌ [데몬 에러] 규격 외 프로토콜입니다.\n".as_bytes()).await;
                        continue;
                    }
                };

                let response_message = match req.cmd.as_str() {
                    "status" => {
                        let uptime_secs = start_time.elapsed().as_secs();
                        format!("GaramOS Daemon Running\nNode Name: {}\nUptime: {}s\nVersion: {}\n", config.hostname, uptime_secs, version)
                    }
                    // 💡 [조정 완료] 한 단계 쪼개진 hardware::disks 폴더 모듈방으로 우회 호출 대행!
                    "disk-list" => hardware::disks::get_json_disks(),
                    "disk-smart" => {
                        if let Some(disk_target) = req.args.get(0) {
                            hardware::disks::get_disk_smart(disk_target)
                        } else {
                            "❌ [에러] 디스크 이름 누락.\n".to_string()
                        }
                    }
                    "pool-create" => {
                        if req.args.len() >= 4 {
                            let pool_name = &req.args[0];
                            let engine_type = &req.args[1];
                            let raid_type = &req.args[2];
                            let disks: Vec<&str> = req.args[3..].iter().map(|s| s.as_str()).collect();

                            let backend: Box<dyn StorageBackend> = match engine_type.as_str() {
                                "zfs"    => Box::new(ZfsBackend),
                                "hybrid" => Box::new(GhsBackend),
                                "lvm"    => Box::new(LvmBackend),
                                _ => {
                                    let _ = stream.write_all("❌ 지원하지 않는 엔진.\n".as_bytes()).await;
                                    continue;
                                }
                            };

                            match backend.create_pool(pool_name, raid_type, &disks) {
                                Ok(report) => report,
                                Err(err_report) => err_report,
                            }
                        } else {
                            "❌ [에러] 인자 부족.\n".to_string()
                        }
                    }
                    "pool-list" => {
                        if let Some(engine_type) = req.args.get(0) {
                            let backend: Box<dyn StorageBackend> = match engine_type.as_str() {
                                "zfs"    => Box::new(ZfsBackend),
                                "hybrid" => Box::new(GhsBackend),
                                "lvm"    => Box::new(LvmBackend),
                                _ => {
                                    let _ = stream.write_all("❌ 알 수 없는 엔진.\n".as_bytes()).await;
                                    continue;
                                }
                            };
                            match backend.list_pools() {
                                Ok(list) => format!("📄 [{}] 활성화 풀 목록:\n{}", engine_type, list),
                                Err(e) => e,
                            }
                        } else {
                            "❌ 엔진 타입 누락.\n".to_string()
                        }
                    }
                    _ => "❌ 알 수 없는 명령어입니다.\n".to_string()
                };

                let _ = stream.write_all(response_message.as_bytes()).await;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config_content = fs::read_to_string("config.toml")
        .expect("⚠️ config.toml 파일을 찾을 수 없습니다!");
    
    let config: SystemConfig = toml::from_str(&config_content)
        .expect("⚠️ config.toml 포맷 에러!");

    let _ = fs::remove_file(&config.socket_path);

    let local = LocalSet::new();
    local.run_until(run_daemon(config)).await;
}