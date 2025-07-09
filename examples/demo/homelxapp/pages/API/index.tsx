import React, { useState } from 'react';
import Button from '../../components/Button';
import StatusIndicator from '../../components/StatusIndicator';
import DeviceInfoDisplay from '../../components/DeviceInfoDisplay';

export default function APIPage() {
  const [navStatus, setNavStatus] = useState('idle');
  const [deviceStatus, setDeviceStatus] = useState('idle');
  const [deviceInfo, setDeviceInfo] = useState({
    brand: '-',
    model: '-',
    system: '-'
  });
  const [showDeviceInfo, setShowDeviceInfo] = useState(false);

  const navigateToMiniProgram = () => {
    setNavStatus('loading');

    try {
      openLxApp({
        appId: 'testminiapp',
        path: 'pages/home/index.html'
      });

      setNavStatus('success');
      setTimeout(() => setNavStatus('idle'), 2000);
    } catch (error) {
      setNavStatus('error');
      setTimeout(() => setNavStatus('idle'), 2000);
    }
  };

  const getDeviceInfo = async () => {
    setDeviceStatus('loading');
    setShowDeviceInfo(false);

    try {
      const deviceInfo = await lx.getDeviceInfo();

      setDeviceInfo({
        brand: deviceInfo.brand || 'Unknown',
        model: deviceInfo.model || 'Unknown',
        system: deviceInfo.system || 'Unknown'
      });

      setDeviceStatus('success');
      setShowDeviceInfo(true);
      // Auto hide success status after showing device info
      setTimeout(() => setDeviceStatus('idle'), 1500);
      console.log('Device Info:', deviceInfo);
    } catch (error) {
      setDeviceStatus('error');
      setShowDeviceInfo(false);
      setTimeout(() => setDeviceStatus('idle'), 2000);
      console.error('Failed to get device info:', error);
    }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div className="page-title">API Capabilities</div>
        <div className="page-desc">LingXia Platform API Demonstrations</div>
      </div>

      <div className="page-body">
        <div className="section-title">Navigation APIs</div>
        <div className="section">
          <div className="cell">
            <div className="cell-icon">🧭</div>
            <div className="cell-body">
              <div className="cell-title">Open LingXia App</div>
              <div className="cell-desc">Navigate to another LxApp</div>
            </div>
            <div className="cell-footer">
              <Button
                onClick={navigateToMiniProgram}
                disabled={navStatus === 'loading'}
                loading={navStatus === 'loading'}
                variant="primary"
                size="small"
              >
                Launch
              </Button>
              <StatusIndicator status={navStatus} />
            </div>
          </div>
        </div>

        <div className="section-title">Device APIs</div>
        <div className="section">
          <div className="cell">
            <div className="cell-icon">📱</div>
            <div className="cell-body">
              <div className="cell-title">Get Device Information</div>
              <div className="cell-desc">Retrieve current device details</div>
            </div>
            <div className="cell-footer">
              <Button
                onClick={getDeviceInfo}
                disabled={deviceStatus === 'loading'}
                loading={deviceStatus === 'loading'}
                variant="secondary"
                size="small"
              >
                Get Info
              </Button>
              <StatusIndicator status={deviceStatus} />
            </div>
          </div>

          {showDeviceInfo && (
            <div className="device-info-wrapper">
              <DeviceInfoDisplay deviceInfo={deviceInfo} />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
