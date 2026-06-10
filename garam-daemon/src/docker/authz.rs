//! GaramOS Enterprise Docker Authorization Guard (Dynamic Policy Block Edition)
//! 
//! [가람 OS 독점 안보 검문소]: 도커 데몬(dockerd)이 컨테이너를 생성하기 직전
//! 가람 데몬의 유닉스 소켓을 강제로 경유하게 하여, 사설 앱 진형에 
//! 설정된 자원 cgroups 쇠창살 격벽을 강제 낙인찍는 보안 엔진.

use axum::{routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::os::unix::fs::PermissionsExt; 
use log::{info, warn};
use tokio::net::UnixListener;
use hyper_util::server::conn::auto;
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use nix::unistd::{chown, Group};
use std::sync::Arc;

#[allow(non_snake_case)]
#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct AuthZReq {
    #[serde(default)]
    pub User: String,
    #[serde(default)]
    pub RequestMethod: String,
    #[serde(default)]
    pub RequestURI: String,
    #[serde(default)]
    pub RequestBody: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Debug)]
pub struct AuthZRes {
    pub Allow: bool,
    pub Msg: String,
    pub ModifiedRequestBody: Option<String>,
}

// 👑 핸들러 분기선까지 정책 컨텍스트를 안전하게 이월하기 위한 내부 상태 캡슐
struct EngineContext {
    enable_auto_mutation: bool,
    default_cpu_limit: f32,
    default_memory_mb: u64,
}

pub struct GaramDockerAuthzEngine;

impl GaramDockerAuthzEngine {
    /// 🏎️ [검문소 테이블 동적 기동 작전]
    /// 👑 [E0061 정밀 사살 완공]: main.rs가 사출하는 6대 유전자를 한 자의 오차도 없이 수령하도록 인터페이스 결속!
    pub async fn start_checkpoint(
        socket_path: &str, 
        socket_mode: u32, 
        group_name: &str,
        enable_auto_mutation: bool,
        default_cpu_limit: f32,
        default_memory_mb: u64
    ) -> Result<(), String> {
        let path = Path::new(socket_path);
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }

        let listener = UnixListener::bind(socket_path)
            .map_err(|e| format!("❌ [도커 안보 붕괴] 테이블 소켓 바인딩 거부: {}", e))?;
        
        if let Ok(metadata) = std::fs::metadata(socket_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(socket_mode); 
            let _ = std::fs::set_permissions(socket_path, perms);
        }

        if let Ok(Some(target_group)) = Group::from_name(group_name) {
            let _ = chown(path, None, Some(target_group.gid));
            info!("🔰 [안보 완공] 도커 Authz 소켓 소유권 '{}' 그룹 귀속 완료.", group_name);
        }

        // 👑 [컨텍스트 공유화]: Axum 상태 저장소에 동적 수치들 매립 완공!
        let context = Arc::new(EngineContext {
            enable_auto_mutation,
            default_cpu_limit,
            default_memory_mb,
        });

        // Axum 라우터 상태 전송 배선 정렬
        let app = Router::new()
            .route("/AuthZPlugin.AuthZReq", post(Self::handle_inspect_request))
            .route("/AuthZPlugin.AuthZRes", post(Self::handle_response_noop))
            .with_state(context); // 🤝 유전자 지도 전달

        info!("🐳 가람OS 도커 안보 통제 플러그인 동적 안착 완공 (모드: 0o{:o}, 자원통제 활성화: {})", socket_mode, enable_auto_mutation);

        let hyper_service = TowerToHyperService::new(app);

        tokio::spawn(async move {
            loop {
                if let Ok((unix_stream, _)) = listener.accept().await {
                    let io = TokioIo::new(unix_stream);
                    let service_clone = hyper_service.clone();
                    tokio::spawn(async move {
                        let _ = auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                            .serve_connection(io, service_clone)
                            .await;
                    });
                }
            }
        });

        Ok(())
    }

    /// 🔍 [도커 생성 통제 메인 핸들러]: config.toml의 설정 수치에 따라 완벽하게 동적 격벽 사출!
    async fn handle_inspect_request(
        axum::extract::State(ctx): axum::extract::State<Arc<EngineContext>>, // 👑 장부 수령
        Json(payload): Json<AuthZReq>
    ) -> Json<AuthZRes> {
        if payload.RequestURI.contains("/containers/create") {
            // 🛡️ [스위치 제어]: enable_auto_mutation이 false이면 자원 인젝션을 통째로 우아하게 스킵!
            if !ctx.enable_auto_mutation {
                info!("🔰 [보안 바이패스] config.toml의 안보 정책에 의거, 자원 제한 변이 생략.");
                return Json(AuthZRes {
                    Allow: true,
                    Msg: "GaramOS Core Security Passed (Mutation Skipped).".to_string(),
                    ModifiedRequestBody: None,
                });
            }

            warn!("🚨 [검문 발동] 사설 앱 컨테이너 생성 시도 감지! 설정 기반 동적 자원 쇠창살 격벽 주입 개시!");

            if let Some(ref body_raw) = payload.RequestBody {
                if let Ok(mut container_json) = serde_json::from_str::<serde_json::Value>(body_raw) {
                    
                    if container_json["HostConfig"].is_null() {
                        container_json["HostConfig"] = serde_json::json!({});
                    }

                    if let Some(host_config) = container_json["HostConfig"].as_object_mut() {
                        // 1. 설정 연동 CPU 제한 주입 (1코어 = 1_000_000_000 NanoCpus)
                        let nano_cpus = (ctx.default_cpu_limit * 1_000_000_000.0) as i64;
                        host_config.insert("NanoCpus".to_string(), serde_json::json!(nano_cpus));

                        // 2. 설정 연동 RAM 제한 주입 (MB단위 -> Byte단위 컨버팅)
                        let memory_bytes = ctx.default_memory_mb * 1024 * 1024;
                        host_config.insert("Memory".to_string(), serde_json::json!(memory_bytes));
                        
                        info!("🟢 [쇠창살 낙인 완공] 설정값 영구 락인: CPU {:.1}코어 | RAM {} MB", ctx.default_cpu_limit, ctx.default_memory_mb);
                    }

                    let modified_body = container_json.to_string();
                    return Json(AuthZRes {
                        Allow: true,
                        Msg: format!("GaramOS 안보법에 의거하여 자원 격벽(CPU: {}도막, RAM: {}MB)이 강제 주입되었습니다.", ctx.default_cpu_limit, ctx.default_memory_mb),
                        ModifiedRequestBody: Some(modified_body),
                    });
                }
            }
        }

        Json(AuthZRes {
            Allow: true,
            Msg: "GaramOS Core Security Passed.".to_string(),
            ModifiedRequestBody: None,
        })
    }

    async fn handle_response_noop(Json(_payload): Json<serde_json::Value>) -> Json<AuthZRes> {
        Json(AuthZRes {
            Allow: true,
            Msg: "Success".to_string(),
            ModifiedRequestBody: None,
        })
    }
}