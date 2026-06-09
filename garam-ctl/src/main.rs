use clap::{Parser, Subcommand, CommandFactory};
use clap_complete::{generate, shells::Bash};

// 🤝 공통 방에서 규격 구조체 소환!
use garam_common::IpcRequest;

#[derive(Parser)]
#[command(name = "garamctl", author = "Ethan Lee", version = "1.0", about = "가람OS 하드웨어 및 데몬 제어 유틸리티", arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Status,
    PoolCreate {
        name: String,
        raid_type: String,
        disks: Vec<String>,
    },
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

    // 공통 구조체 데이터 매핑 빌드
    let ipc_req = match &cli.command {
        Commands::Status => IpcRequest { cmd: "status".to_string(), args: vec![] },
        Commands::PoolCreate { name, raid_type, disks } => {
            let mut args = vec![name.clone(), raid_type.clone()];
            args.extend(disks.clone());
            IpcRequest { cmd: "pool-create".to_string(), args }
        }
        Commands::Disks { command } => match command {
            DisksCommands::List => IpcRequest { cmd: "disk-list".to_string(), args: vec![] },
            DisksCommands::Scan => IpcRequest { cmd: "disk-scan".to_string(), args: vec![] },
            DisksCommands::Smart { name } => IpcRequest { cmd: "disk-smart".to_string(), args: vec![name.clone()] },
        },
        Commands::Complete => {
            let mut cmd = Cli::command();
            generate(Bash, &mut cmd, "garamctl", &mut std::io::stdout());
            return;
        }
    };

    let socket_path = "/tmp/garamd.sock";

    // 🤝 공통 라이브러리 마스터 함수 딱 한 줄로 통신 종결!
    match garam_common::send_ipc_request(socket_path, &ipc_req).await {
        Ok(response) => print!("{}", response),
        Err(err_msg) => println!("{}", err_msg),
    }
}