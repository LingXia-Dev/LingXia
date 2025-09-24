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

  const formatCoordinate = (
    value: number | null | undefined,
    axis: 'latitude' | 'longitude'
  ) => {
    if (value === null || value === undefined) {
      return '--';
    }

    const absolute = Math.abs(value);
    const degrees = Math.floor(absolute);
    const minutes = Math.floor((absolute - degrees) * 60);
    const direction = axis === 'latitude'
      ? value >= 0 ? 'N' : 'S'
      : value >= 0 ? 'E' : 'W';

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
                  {formatCoordinate(location.longitude, 'longitude')}{' '}
                  {formatCoordinate(location.latitude, 'latitude')}
                </div>
                
                {/* Location Details */}
                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 mt-6 text-sm">
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

      </div>
    </div>
  );
}
