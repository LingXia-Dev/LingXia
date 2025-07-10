import React from 'react';

interface DeviceInfo {
  brand: string;
  model: string;
  system: string;
}

interface DeviceInfoDisplayProps {
  deviceInfo: DeviceInfo;
  className?: string;
}

export default function DeviceInfoDisplay({ deviceInfo, className = '' }: DeviceInfoDisplayProps) {
  const infoItems = [
    { label: 'Brand', value: deviceInfo.brand },
    { label: 'Model', value: deviceInfo.model },
    { label: 'System', value: deviceInfo.system }
  ];

  return (
    <div className={`device-info-card ${className}`}>
      {infoItems.map((item, index) => (
        <div key={index} className="device-info-item">
          <span className="device-info-label">{item.label}</span>
          <span className="device-info-value">{item.value}</span>
        </div>
      ))}
    </div>
  );
}
