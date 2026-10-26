import { type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, Card, Icon, Page } from '../../shared/ui';

type Location = { longitude?: number; latitude?: number; accuracy?: number; altitude?: number; speed?: number };
function coordinate(value: number | undefined, latitude: boolean) {
  if (value == null || !Number.isFinite(value)) return '--';
  const absolute = Math.abs(value),
    degrees = Math.floor(absolute),
    minutes = Math.floor((absolute - degrees) * 60);
  const hemisphere = latitude ? (value >= 0 ? 'N' : 'S') : value >= 0 ? 'E' : 'W';
  return `${hemisphere}: ${degrees}°${String(minutes).padStart(2, '0')}'`;
}
const decimal = (value: number | undefined, digits: number, suffix = '') =>
  value == null || !Number.isFinite(value) ? '--' : `${value.toFixed(digits)}${suffix}`;

export function render({ data, actions }: View) {
  const location = data.location as Location | null;
  const details = location
    ? [
        ['Longitude', decimal(location.longitude, 6)],
        ['Latitude', decimal(location.latitude, 6)],
        ['Accuracy', decimal(location.accuracy, 1, 'm')],
        ['Altitude', decimal(location.altitude, 1, 'm')],
        ['Speed', decimal(location.speed, 1, 'm/s')],
      ]
    : [];
  return (
    <Page title="getLocation">
      <Card testId="location-card" className="p-6 gap-4 items-center">
        <text className="text-base text-muted">Current Location</text>
        {data.isLoading ? (
          <row className="items-center gap-3 py-8">
            <Icon name="location" className="w-8 h-8 text-accent" />
            <text testId="location-status" className="text-base text-muted">
              Getting location...
            </text>
          </row>
        ) : location ? (
          <column testId="location-result" className="w-full gap-4 items-center">
            <text
              testId="location-coordinate"
              className="text-2xl font-light"
            >{`${coordinate(location.longitude, false)} ${coordinate(location.latitude, true)}`}</text>
            <grid className="w-full gap-4 grid-cols-1 sm:grid-cols-2 lg:grid-cols-3">
              {details.map(([label, value]) => (
                <column key={label} className="items-center gap-1">
                  <text className="text-sm text-muted">{label}</text>
                  <text testId={`location-${label.toLowerCase()}`} className="text-sm font-medium">
                    {value}
                  </text>
                </column>
              ))}
            </grid>
          </column>
        ) : (
          <text testId="location-status" className={data.locationError ? 'py-8 text-base text-danger' : 'py-8 text-base text-muted'}>
            {String(data.locationError || 'No location data available')}
          </text>
        )}
      </Card>
      <Button testId="location-get" disabled={!!data.isLoading} onClick={() => actions.getLocation()}>
        {data.isLoading ? 'Getting Location...' : 'Get Location'}
      </Button>
      <Button variant="secondary" testId="location-clear" disabled={!!data.isLoading} onClick={() => actions.clearLocation()}>
        Clear
      </Button>
    </Page>
  );
}
