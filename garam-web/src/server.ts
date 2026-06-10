import net from 'net';
import express from 'express';
import cors from 'cors';
import path from 'path';
import { fileURLToPath } from 'url';
import http from 'http';
import { WebSocketServer } from 'ws';

const app = express();
const PORT = 8080;
const DAEMON_SOCKET_PATH = '/tmp/garam-daemon.sock';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// 미들웨어
app.use(cors());
app.use(express.json());

// 1. HTTP 서버 및 웹소켓 서버 구축
const server = http.createServer(app);
// server.ts 의 15번 줄 부근 수정
const wss = new WebSocketServer({ 
  server,
  // 브라우저 접속을 확실히 수용하기 위한 설정
  clientTracking: true,
  perMessageDeflate: false 
});

// 2. [IPC IPC] 기존 4바이트 코덱 요청/응답 파이프라인
function sendDaemonIpc(cmd: string, args: string[] = []): Promise<any> {
  return new Promise((resolve, reject) => {
    const client = net.createConnection({ path: DAEMON_SOCKET_PATH });
    const payloadStr = JSON.stringify({ cmd, args });
    const payloadBuffer = Buffer.from(payloadStr, 'utf-8');
    const headerBuffer = Buffer.alloc(4);
    headerBuffer.writeUInt32BE(payloadBuffer.length, 0);

    client.write(Buffer.concat([headerBuffer, payloadBuffer]));

    let responseBuffer = Buffer.alloc(0);
    let expectedLength = -1;

    client.on('data', (chunk) => {
      responseBuffer = Buffer.concat([responseBuffer, chunk]);
      if (expectedLength === -1 && responseBuffer.length >= 4) {
        expectedLength = responseBuffer.readUInt32BE(0);
        responseBuffer = responseBuffer.subarray(4);
      }
      if (expectedLength !== -1 && responseBuffer.length >= expectedLength) {
        try {
          resolve(JSON.parse(responseBuffer.subarray(0, expectedLength).toString('utf-8')));
        } catch (err) {
          reject(err);
        } finally {
          client.end();
        }
      }
    });
    client.on('error', (err) => reject(err));
  });
}

// 3. 실시간 웹소켓 관제 브릿지
// 3. 실시간 웹소켓 관제 브릿지 (수정된 부분)
wss.on('connection', (ws) => {
  console.log('🔗 [Websocket] 관제소 연결 수립 완료');
  
  const daemonClient = net.createConnection({ path: DAEMON_SOCKET_PATH });
  let buffer = Buffer.alloc(0);

  // server.ts - daemonClient.on('connect') 부분 수정
    // server.ts 의 connect 내부
    daemonClient.on('connect', () => {
    console.log('✅ 데몬과 UDS 도킹 성공!');
    
    // 명령어 객체
    const cmdObj = { cmd: 'status', args: ['w'] };
    const payload = Buffer.from(JSON.stringify(cmdObj), 'utf-8');
    
    // 4바이트 길이 헤더 (Big Endian)
    const header = Buffer.alloc(4);
    header.writeUInt32BE(payload.length, 0);
    
    // 헤더 + 페이로드 한번에 전송
    const packet = Buffer.concat([header, payload]);
    daemonClient.write(packet);
    console.log('🚀 [IPC] 명령어 투하 완료:', packet.length, 'bytes');
    });

 // server.ts - daemonClient.on('data') 내부를 이 로직으로 교체
daemonClient.on('data', (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);

  while (buffer.length > 0) {
    // 1. JSON 객체의 시작 '{' 위치를 찾습니다.
    const startIdx = buffer.indexOf('{');
    
    // 2. '{'가 없다면 데이터가 덜 왔거나 쓰레기 값이므로 버퍼 비움
    if (startIdx === -1) {
      buffer = Buffer.alloc(0);
      break;
    }

    // 3. '{'부터 끝까지 일단 남겨둡니다.
    const potentialJson = buffer.subarray(startIdx).toString('utf-8');
    
    try {
      // 4. JSON 파싱 시도
      const parsed = JSON.parse(potentialJson);
      if (ws.readyState === 1) ws.send(JSON.stringify(parsed));
      
      // 파싱 성공했으면 버퍼에서 해당 부분 삭제
      buffer = Buffer.alloc(0); 
      break;
    } catch (e) {
      // 파싱 실패 시, 데이터가 더 들어와야 하는 상황일 수 있으므로 대기
      break; 
    }
  }
});

  ws.on('close', () => daemonClient.end());
  ws.on('error', (e) => console.error("🚨 웹소켓 내부 에러:", e));
  daemonClient.on('error', (e) => console.error("🚨 데몬 통신 에러:", e));
});

// 4. API 밸브
app.get('/api/v1/status', async (_req, res) => {
  try {
    const daemonResponse = await sendDaemonIpc('status');
    res.json(daemonResponse);
  } catch (error: any) {
    res.status(503).json({ error: error.message });
  }
});

// 5. 프론트엔드 정적 파일 서빙
const publicPath = path.join(__dirname, 'public');
app.use(express.static(publicPath));
app.get('*', (_req, res) => res.sendFile(path.join(publicPath, 'index.html')));

server.listen(PORT, () => console.log(`🛡️ 가람OS 일체형 통합 웹 서버 개통 (Port: ${PORT})`));