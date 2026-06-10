import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173, // 프론트엔드 포트 고정
    proxy: {
      // 리액트가 /api로 시작하는 요청을 쏘면
      '/api': {
        target: 'http://localhost:8080', // 8080 백엔드로 텔레포트!
        changeOrigin: true,
        secure: false,
      },
    },
  },
});