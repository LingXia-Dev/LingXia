import { type View } from '@lingxia/ui';
import '../../theme/index.css';

type Location = { longitude?: number; latitude?: number; accuracy?: number; altitude?: number; speed?: number };
function coordinate(value: number | undefined, latitude: boolean) {
  if (value == null || !Number.isFinite(value)) return '--';
  const absolute = Math.abs(value);
  const degrees = Math.floor(absolute);
  const minutes = Math.floor((absolute - degrees) * 60);
  return `${latitude ? value >= 0 ? 'N' : 'S' : value >= 0 ? 'E' : 'W'}: ${degrees}°${String(minutes).padStart(2, '0')}'`;
}
function decimal(value: number | undefined, digits: number, suffix = '') {
  return value == null || !Number.isFinite(value) ? '--' : `${value.toFixed(digits)}${suffix}`;
}
export function render({ data, actions, env }: View) {
  const location = data.location as Location | null;
  const height = Math.max(1, env.viewport.height - env.insets.top - Math.max(env.insets.bottom, env.keyboard.height));
  const width = Math.max(1, env.viewport.width - env.insets.left - env.insets.right);
  const details = location ? [
    ['Longitude', decimal(location.longitude, 6)], ['Latitude', decimal(location.latitude, 6)],
    ['Accuracy', decimal(location.accuracy, 1, 'm')], ['Altitude', decimal(location.altitude, 1, 'm')], ['Speed', decimal(location.speed, 1, 'm/s')],
  ] : [];
  return <column className="bg-surface-100 text-gray-800">
    <spacer style={{ height: env.insets.top }} />
    <row style={{ height }}><spacer style={{ width: env.insets.left }} />
      <scroll testId="page-scroll" className="grow basis-0 min-w-0" style={{ height }}>
        <column className="px-4 py-6 gap-8">
          <column className="items-center gap-2"><text className="text-2xl font-light">getLocation</text><spacer className="w-16 h-0.5 bg-surface-400" /></column>
          <column testId="location-card" className="bg-surface rounded-lg shadow-sm p-6 text-center gap-4">
            <text className="text-base text-gray-600">Current Location</text>
            {data.isLoading ? <row className="items-center justify-center gap-3 py-8">
              <vector className="w-8 h-8 text-blue-500" paths={[{ d: 'M20 12A8 8 0 1 1 12 4', fill: 'none', stroke: 'currentColor', strokeWidth: 2 }]} />
              <text testId="location-status" className="text-base text-gray-600">Getting location...</text>
            </row> : location ? <column testId="location-result" className="gap-4">
              <text testId="location-coordinate" className="text-2xl font-light">{`${coordinate(location.longitude, false)} ${coordinate(location.latitude, true)}`}</text>
              <grid columns={width >= 1024 ? 3 : width >= 640 ? 2 : 1} className="gap-4 pt-2">
                {details.map(([label, value]) => <column key={label} className="items-center gap-1 text-center"><text className="text-sm text-gray-500">{label}</text><text testId={`location-${label.toLowerCase()}`} className="text-sm font-medium">{value}</text></column>)}
              </grid>
            </column> : <text testId="location-status" className={data.locationError ? 'py-8 text-base text-red-500 dark:text-red-400' : 'py-8 text-base text-gray-500'}>{String(data.locationError || 'No location data available')}</text>}
          </column>
          <column className="gap-3">
            <button testId="location-get" enabled={!data.isLoading} onClick={() => actions.getLocation()} className={data.isLoading ? 'min-h-11 py-4 px-6 rounded-lg bg-surface-400 text-white font-medium text-base' : 'min-h-11 py-4 px-6 rounded-lg bg-blue-500 text-white font-medium text-base'}>{data.isLoading ? 'Getting Location...' : 'Get Location'}</button>
            <button testId="location-clear" enabled={!data.isLoading} onClick={() => actions.clearLocation()} className="min-h-11 py-4 px-6 rounded-lg bg-surface text-gray-700 font-medium text-base border border-line-300">Clear</button>
          </column>
        </column>
      </scroll><spacer style={{ width: env.insets.right }} />
    </row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
