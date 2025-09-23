import React from 'react';
import '../../tailwind.css';

export default function LocationPage() {
  // Use LingXia hook to get data and functions
  const { data, getLocation, clearLocation } = window.useLingXia();
  const { location = null, isLoading = false } = data;

  React.useEffect(() => {
    document.body.className = 'location-page';
    return () => {
      document.body.className = '';
    };
  }, []);

  const formatCoordinate = (value) => {
    if (!value) return '--';
    const degrees = Math.floor(Math.abs(value));
    const minutes = Math.floor((Math.abs(value) - degrees) * 60);
    const direction = value >= 0 ? (degrees === Math.floor(value) ? 'E' : 'N') : (degrees === Math.floor(Math.abs(value)) ? 'W' : 'S');
    return `${direction}: ${degrees}°${minutes.toString().padStart(2, '0')}'`;
  };

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="px-4 py-6">
        
        {/* Header */}
        <div className="text-center mb-8">
          <h1 className="text-2xl font-light text-gray-800 mb-2">getLocation</h1>
          <div className="w-16 h-0.5 bg-gray-400 mx-auto"></div>
        </div>

        {/* Location Display */}
        <div className="bg-white rounded-lg shadow-sm p-6 mb-8">
          <div className="text-center">
            <div className="text-gray-600 mb-4">Current Location</div>
            
            {isLoading ? (
              <div className="flex items-center justify-center py-8">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
                <span className="ml-3 text-gray-600">Getting location...</span>
              </div>
            ) : location ? (
              <div className="space-y-4">
                <div className="text-2xl font-light text-gray-800">
                  {formatCoordinate(location.longitude)} {formatCoordinate(location.latitude)}
                </div>
                
                {/* Location Details */}
                <div className="grid grid-cols-2 gap-4 mt-6 text-sm">
                  <div className="text-center">
                    <div className="text-gray-500">Longitude</div>
                    <div className="font-medium">{location.longitude?.toFixed(6) || '--'}</div>
                  </div>
                  <div className="text-center">
                    <div className="text-gray-500">Latitude</div>
                    <div className="font-medium">{location.latitude?.toFixed(6) || '--'}</div>
                  </div>
                  <div className="text-center">
                    <div className="text-gray-500">Accuracy</div>
                    <div className="font-medium">{location.accuracy ? `${location.accuracy.toFixed(1)}m` : '--'}</div>
                  </div>
                  <div className="text-center">
                    <div className="text-gray-500">Altitude</div>
                    <div className="font-medium">{location.altitude ? `${location.altitude.toFixed(1)}m` : '--'}</div>
                  </div>
                  <div className="text-center">
                    <div className="text-gray-500">Speed</div>
                    <div className="font-medium">{location.speed ? `${location.speed.toFixed(1)}m/s` : '--'}</div>
                  </div>
                  <div className="text-center">
                    <div className="text-gray-500">Coordinate Type</div>
                    <div className="font-medium">{location.coordinate_type || location.type || '--'}</div>
                  </div>
                </div>
              </div>
            ) : (
              <div className="py-8 text-gray-500">
                No location data available
              </div>
            )}
          </div>
        </div>

        {/* Action Buttons */}
        <div className="space-y-3">
          <button
            onClick={getLocation}
            disabled={isLoading}
            className="w-full bg-green-500 hover:bg-green-600 disabled:bg-gray-400 text-white font-medium py-4 px-6 rounded-lg transition-colors"
          >
            {isLoading ? 'Getting Location...' : 'Get Location'}
          </button>
          
          <button
            onClick={clearLocation}
            className="w-full bg-white hover:bg-gray-50 text-gray-700 font-medium py-4 px-6 rounded-lg border border-gray-300 transition-colors"
          >
            Clear
          </button>
        </div>

        {/* Settings Icon (placeholder) */}
        <div className="fixed bottom-6 right-6">
          <div className="w-12 h-12 bg-white rounded-full shadow-lg flex items-center justify-center">
            <svg className="w-6 h-6 text-gray-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
              <circle cx="12" cy="12" r="3"/>
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1 1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
            </svg>
          </div>
        </div>
      </div>
    </div>
  );
}
