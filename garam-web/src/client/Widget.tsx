import React from 'react';

interface WidgetProps {
  title: string;
  value: string | number;
  unit: string;
  icon: string;
  color: string;
}

export const DashboardWidget: React.FC<WidgetProps> = ({ title, value, unit, icon, color }) => (
  <div style={{
    background: '#1c1c1c',
    padding: '20px',
    borderRadius: '12px',
    border: '1px solid #333',
    display: 'flex',
    alignItems: 'center',
    gap: '15px',
    boxShadow: '0 4px 6px rgba(0,0,0,0.3)'
  }}>
    <div style={{ fontSize: '24px' }}>{icon}</div>
    <div>
      <div style={{ color: '#aaa', fontSize: '12px', marginBottom: '4px' }}>{title}</div>
      <div style={{ fontSize: '20px', fontWeight: 'bold', color }}>{value}<span style={{ fontSize: '14px', marginLeft: '4px', color: '#666' }}>{unit}</span></div>
    </div>
  </div>
);