import { useState, useEffect } from 'react';
import { DashboardWidget } from './client/Widget';

export default function App() {
  const [status, setStatus] = useState<any>(null);

  useEffect(() => {
    // 1. WebSocket 서버 접속 (백엔드 8080 포트와 같은 주소)
    // 수정 전: const ws = new WebSocket(`ws://${window.location.host}`);
    // 주소가 100.102.218.53:5173으로 잡히고 있습니다.

    // 수정 후: 8080 포트로 강제 연결!
    const ws = new WebSocket('ws://100.102.218.53:8080');
    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        setStatus(data); // 데이터가 날아올 때마다 즉시 갱신!
      } catch (e) {
        console.error("🚨 데이터 파싱 오류:", e);
      }
    };

    ws.onerror = (err) => console.error("🚨 관제소 연결 오류:", err);
    ws.onclose = () => console.log("🔌 관제소 연결 종료");

    return () => ws.close(); // 컴포넌트 정리 시 소켓 자동 해제
  }, []);

  if (!status) return <div style={{color: '#fff'}}>📡 관제소 신호 동기화 중...</div>;

  return (
    <div style={{ padding: '40px', background: '#111', minHeight: '100vh', color: '#fff' }}>
      <h1>🛰️ GaramOS Dashboard (Real-time)</h1>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '20px', marginTop: '30px' }}>
        <DashboardWidget title="CPU 사용량" value={status.hardware.cpu_usage_pct.toFixed(1)}unit="%" icon="🏎️" color="#00ffcc" />
        <DashboardWidget title="RAM 사용량" value={status.hardware.ram_usage_pct} unit="%" icon="🧠" color="#ffcc00" />
        <DashboardWidget title="UPS 배터리" value={status.power_ups.battery_pct} unit="%" icon="🔋" color="#3399ff" />
      </div>
    </div>
  );
}