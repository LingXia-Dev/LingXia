import React, { useState } from 'react';

export default function APIPage() {
  const [navOpen, setNavOpen] = useState(false);
  const [deviceOpen, setDeviceOpen] = useState(false);
  const [navStatus, setNavStatus] = useState('');
  const [deviceStatus, setDeviceStatus] = useState('');
  const [deviceInfo, setDeviceInfo] = useState({
    brand: '-',
    model: '-',
    system: '-'
  });
  const [showDeviceInfo, setShowDeviceInfo] = useState(false);

  const navigateToMiniProgram = () => {
    setNavStatus('loading');

    try {
      // Call the page function
      openLxApp({
        appId: 'testminiapp',
        path: 'pages/home/index.html'
      });

      setNavStatus('success');
    } catch (error) {
      setNavStatus('error');
    }
  };

  const getDeviceInfo = async () => {
    setDeviceStatus('loading');
    setShowDeviceInfo(false);

    try {
      // Call the global lx function
      const deviceInfo = await lx.getDeviceInfo();

      setDeviceInfo({
        brand: deviceInfo.brand || 'Unknown',
        model: deviceInfo.model || 'Unknown',
        system: deviceInfo.system || 'Unknown'
      });

      setDeviceStatus('success');
      setShowDeviceInfo(true);

      console.log('Device Info:', deviceInfo);
    } catch (error) {
      setDeviceStatus('error');
      setShowDeviceInfo(false);
      console.error('Failed to get device info:', error);
    }
  };

  const getStatusText = (status: string) => {
    switch (status) {
      case 'loading': return 'Loading...';
      case 'success': return 'Success';
      case 'error': return 'Failed';
      default: return '';
    }
  };

  return (
    <div className="container">
      <div className="header">
        <div className="icon">⚏</div>
        <p>The following demonstrates some of the key API capabilities available in the LingXia platform.</p>
      </div>

      <div className="category">
        <div className="category-header" onClick={() => setNavOpen(!navOpen)}>
          <div className="category-icon">🧭</div>
          <span className="category-title">Navigation</span>
          <div className="category-toggle">{navOpen ? '⌄' : '›'}</div>
        </div>
        {navOpen && (
          <div className="category-content">
            <div className="api-item" onClick={navigateToMiniProgram}>
              <span className="api-name">Open Mini Program</span>
              <div className={`api-status ${navStatus}`}>
                {getStatusText(navStatus)}
              </div>
            </div>
          </div>
        )}
      </div>

      <div className="category">
        <div className="category-header" onClick={() => setDeviceOpen(!deviceOpen)}>
          <div className="category-icon">📱</div>
          <span className="category-title">Device</span>
          <div className="category-toggle">{deviceOpen ? '⌄' : '›'}</div>
        </div>
        {deviceOpen && (
          <div className="category-content">
            <div className="api-item" onClick={getDeviceInfo}>
              <span className="api-name">Device Information</span>
              <div className={`api-status ${deviceStatus}`}>
                {getStatusText(deviceStatus)}
              </div>
              {showDeviceInfo && (
                <div className="device-info">
                  <div className="device-info-item">
                    <span className="device-info-label">Brand:</span>
                    <span className="device-info-value">{deviceInfo.brand}</span>
                  </div>
                  <div className="device-info-item">
                    <span className="device-info-label">Model:</span>
                    <span className="device-info-value">{deviceInfo.model}</span>
                  </div>
                  <div className="device-info-item">
                    <span className="device-info-label">System:</span>
                    <span className="device-info-value">{deviceInfo.system}</span>
                  </div>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
