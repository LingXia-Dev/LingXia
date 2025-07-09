import React from 'react';

interface StatusIndicatorProps {
  status: 'idle' | 'loading' | 'success' | 'error';
  className?: string;
}

export default function StatusIndicator({ status, className = '' }: StatusIndicatorProps) {
  const getStatusConfig = (status: string) => {
    switch (status) {
      case 'loading':
        return { text: 'Loading...', icon: '⟳', class: 'status-loading' };
      case 'success':
        return { text: 'Success', icon: '✓', class: 'status-success' };
      case 'error':
        return { text: 'Failed', icon: '✗', class: 'status-error' };
      default:
        return { text: '', icon: '', class: 'status-idle' };
    }
  };

  const config = getStatusConfig(status);

  if (status === 'idle') {
    return null;
  }

  return (
    <div className={`status-indicator ${config.class} ${className}`}>
      <span className="status-icon">{config.icon}</span>
      <span className="status-text">{config.text}</span>
    </div>
  );
}
