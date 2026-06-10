use clap::{Parser, Subcommand, CommandFactory};
use clap_complete::{generate, shells::Bash};
use serde_json::Value;
use std::io::Write;

use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tokio_util::bytes::Bytes;
use futures::{StreamExt, SinkExt};

use garam_common::{IpcRequest, send_ipc_request};

#[derive(Parser)]
#[command(name = "garamctl", author = "Ethan Lee", version = "1.0", about = "가람OS 하드웨어 및 데몬 제어 유틸리티", arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 📊 시스템 핵심 자원 전용 (실시간 관제용 --watch 플래그 긴급 투입!)
    Status {
        #[arg(short, long, help = "매초 실시간으로 대시보드를 리프레시하며 추적합니다")]
        watch: bool,
    },
    
    /// 🔌 무정전 전원 관리 및 AC 선로 검문
    Power,  
    
    /// 🏗️ LVM/ZFS 스토리지 풀 원자적 매립 격발
    PoolCreate {
        name: String,
        #[arg(short, long)]
        engine: String,
        #[arg(short, long)]
        raid: String,
        disks: Vec<String>,
    },
    
    /// 💾 실물 드라이브 제어 서브시스템
    Disks {
        #[command(subcommand)]
        command: DisksCommands,
    },
    Complete,
}

#[derive(Subcommand)]
enum DisksCommands {
    List,
    Scan,
    Smart {
        name: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = match Cli::try_parse() {
        Ok(model) => model,
        Err(e) => {
            let error_msg = e.to_string().lines().next().unwrap_or("알 수 없는 명령어 에러").to_string();
            println!("🔴 [가람OS 명령어 에러] {}", error_msg);
            println!("--------------------------------------------------");
            let clean_help = e.to_string().lines().skip(1).collect::<Vec<&str>>().join("\n");
            if clean_help.trim().is_empty() {
                let mut cmd = Cli::command();
                let _ = cmd.print_help();
            } else {
                println!("{}", clean_help.trim());
            }
            println!();
            return;
        }
    };

    let ipc_req = match &cli.command {
        Commands::Status { watch } => {
            let args = if *watch { vec!["w".to_string()] } else { vec![] };
            IpcRequest { cmd: "status".to_string(), args }
        },
        Commands::Power => IpcRequest { cmd: "status".to_string(), args: vec![] },
        Commands::PoolCreate { name, engine, raid, disks } => {
            let mut args = vec![name.clone(), "-e".to_string(), engine.clone(), "-r".to_string(), raid.clone()];
            args.extend(disks.clone());
            IpcRequest { cmd: "create_pool".to_string(), args }
        }
        Commands::Disks { command } => match command {
            DisksCommands::List => IpcRequest { cmd: "disk_list".to_string(), args: vec![] },
            DisksCommands::Scan => IpcRequest { cmd: "disk_scan".to_string(), args: vec![] },
            DisksCommands::Smart { name } => IpcRequest { cmd: "disk_smart".to_string(), args: vec![name.clone()] },
        },
        Commands::Complete => {
            let mut cmd = Cli::command();
            generate(Bash, &mut cmd, "garamctl", &mut std::io::stdout());
            return;
        }
    };

    let socket_path = "/tmp/garam-daemon.sock";

    // 🏎️ [실시간 대시보드 인터셉터 분기선]
    if let Commands::Status { watch: true } = &cli.command {
        print!("\x1B[?25l"); // 커서 숨기기
        let _ = std::io::stdout().flush();

        if let Ok(stream) = tokio::net::UnixStream::connect(socket_path).await {
            let (reader, writer) = tokio::io::split(stream);
            let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());
            let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());

            // ✅ [수정] 수동 패킷 조립 제거 - FramedWrite가 length prefix 처리
            let payload = Bytes::from(serde_json::to_vec(&ipc_req).unwrap());
            if framed_writer.send(payload).await.is_err() {
                println!("🔴 [경고] 데몬 명령어 전송 실패!");
                print!("\x1B[?25h");
                return;
            }

            // 스트리밍 루프
            while let Some(Ok(bytes_mut)) = framed_reader.next().await {
                let response = String::from_utf8_lossy(&bytes_mut);
                if response.trim().starts_with('{') {
                    print!("\x1B[H\x1B[J"); // 화면 클리어
                    display_system_status(response.trim());
                    let _ = std::io::stdout().flush();
                }
            }
        } else {
            println!("🔴 [경고] 데몬 소켓 연결 실패!");
        }
        
        print!("\x1B[?25h"); // 커서 복구
        return;
    }

    // ⚙️ [단발성 분기 처리부]
    match send_ipc_request(socket_path, &ipc_req).await {
        Ok(response) => {
            match &cli.command {
                Commands::Status { watch: false } => display_system_status(&response),
                Commands::Power => display_power_status(&response),
                Commands::Disks { command: DisksCommands::List } => display_disk_list(&response),
                _ => print!("{}", response),
            }
        }
        Err(err_msg) => println!("🔴 관제탑 사출 거부: {}", err_msg),
    }
}

// =========================================================================
// 📊 뷰어 레이어
// =========================================================================

fn display_system_status(json_raw: &str) {
    if let Ok(v) = serde_json::from_str::<Value>(json_raw) {
        println!("========================================================");
        println!(" 📊 GaramOS 시스템 실시간 리소스 관제판 (Status 2.0)");
        println!("========================================================");
        println!(" 🏢 호스트 네임   : {}", v["hostname"].as_str().unwrap_or("Unknown"));
        
        let total_secs = v["uptime_secs"].as_u64().unwrap_or(0);
        let uptime_string = garam_common::format_uptime_human_readable(total_secs);
        println!(" ⏱️ 엔진 가동 시간 : {}", uptime_string);
        println!(" 🎯 커널 소프트버전: v{}", v["version"].as_str().unwrap_or("0.1.0"));
        println!("--------------------------------------------------------");
        
        let hw = &v["hardware"];
        let cpu_val = hw["cpu_usage_pct"].as_f64().unwrap_or(0.0) as usize;
        let cpu_bar = garam_common::make_text_gauge(cpu_val, 10);
        
        println!(" 🏎️ 실시간 CPU 상태 : [{} ] {} %", cpu_bar, cpu_val);
        println!(" 🧠 메모리 사용 스케일: {}% ({} MB / {} MB)", 
            hw["ram_usage_pct"], hw["ram_used_mb"], hw["ram_total_mb"]
        );
        println!(" 🌡️ 드라이브 실물온도: {}", hw["disk_temp"].as_str().unwrap_or("N/A"));
        println!("========================================================");
    } else {
        println!("{}", json_raw);
    }
}

fn display_power_status(json_raw: &str) {
    if let Ok(v) = serde_json::from_str::<Value>(json_raw) {
        let ups = &v["power_ups"];
        let ac_connected = ups["ac_connected"].as_bool().unwrap_or(true);
        
        println!("========================================================");
        println!(" 🔌 GaramOS 엔터프라이즈 무정전 전원 관리자 (Power)");
        println!("========================================================");
        if ac_connected {
            println!(" 🔌 메인 전원 선로 : 🟢 AC ON (한전 시중 전력 정상 공급 중)");
        } else {
            println!(" 🔌 메인 전원 선로 : 🚨 AC OFF (정전 비상 상황!! 배터리 구동)");
        }
        println!("--------------------------------------------------------");
        
        let bat_val = ups["battery_pct"].as_u64().unwrap_or(0) as usize;
        let bat_bar = garam_common::make_text_gauge(bat_val, 10);
        println!(" 🔋 인산철 배터리 잔량: [{} ] {} %", bat_bar, bat_val);
        println!(" ⚡ 실시간 실물 전압  : {:.2} V (리튬 인산철 4S 규격)", ups["voltage"].as_f64().unwrap_or(0.0));
        println!(" ⏳ 비상 대피 골든타임: 약 {} 분 남음", ups["runtime_left_min"]);
        println!("========================================================");
    } else {
        println!("{}", json_raw);
    }
}

fn display_disk_list(json_raw: &str) {
    if let Ok(v) = serde_json::from_str::<Value>(json_raw) {
        println!("------------------------------------------------------");
        println!("   가람OS 순정 커널 실물 스토리지 명부 (lsblk 완전 배제) ");
        println!("------------------------------------------------------");
        println!("   디스크명         |   타입   |   관제 상태 ");
        println!("------------------------------------------------------");
        
        if let Some(disks_array) = v["disks"].as_array() {
            for disk in disks_array {
                if let Some(disk_name) = disk.as_str() {
                    let note = if disk_name == "sda" { "🔒 SYSTEM OS ROLE" } else { "可 (데이터 풀 가용)" };
                    println!(
                        "   /dev/{:<8} |   disk   |  {}",
                        disk_name, note
                    );
                }
            }
        }
        println!("------------------------------------------------------");
    } else {
        println!("{}", json_raw);
    }
}