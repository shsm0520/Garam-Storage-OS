use serde::{Serialize, Deserialize};
use tokio::net::UnixStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// 🔓 외부(ctl, web)에서 마음대로 패킹해서 쓸 수 있도록 pub 마킹!
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IpcRequest {
    pub cmd: String,
    pub args: Vec<String>,
}

// 📡 CLI 전송기와 웹 서버가 데몬 노크할 때 공용으로 쓸 찐 소켓 파이프라인 함수
pub async fn send_ipc_request(socket_path: &str, req: &IpcRequest) -> Result<String, String> {
    // 1. 요청 구조체를 JSON 문자열로 직렬화
    let json_payload = serde_json::to_string(req)
        .map_err(|e| format!("❌ 프로토콜 패킹 실패: {}", e))?;

    // 2. 데몬 소켓 관로에 스트림 연결
    let mut stream = UnixStream::connect(socket_path).await
        .map_err(|_| "🔴 가람 백엔드 데몬 엔진이 꺼져 있거나 응답하지 않습니다.".to_string())?;

    // 3. JSON 데이터 쏴주기
    stream.write_all(json_payload.as_bytes()).await
        .map_err(|e| format!("❌ 소켓 송신 에러: {}", e))?;
    
    // 4. 데몬이 하드웨어 주무르고 보낸 응답 리포트 받아내기
    let mut response = [0; 4096];
    let n = stream.read(&mut response).await
        .map_err(|e| format!("❌ 소켓 수신 에러: {}", e))?;

    Ok(String::from_utf8_lossy(&response[..n]).into_owned())
}