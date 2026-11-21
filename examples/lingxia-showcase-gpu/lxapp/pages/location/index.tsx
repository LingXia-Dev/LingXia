import { t, type View } from '@lingxia/view';
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
    <Page>
      <Card testId="location-card" className="p-6 gap-4 items-center">
        <text className="text-base text-muted">{t('location.currentLocation')}</text>
        {data.isLoading ? (
          <view className="flex-row items-center gap-3 py-8">
            <Icon name="location" className="w-8 h-8 text-accent" />
            <text data-testid="location-status" className="text-base text-muted">
              {t('location.gettingLocation')}
            </text>
          </view>
        ) : location ? (
          <view data-testid="location-result" className="w-full gap-4 items-center">
            <text
              data-testid="location-coordinate"
              className="text-2xl font-light"
            >{`${coordinate(location.longitude, false)} ${coordinate(location.latitude, true)}`}</text>
            <view className="grid w-full gap-4 grid-cols-1 sm:grid-cols-2 lg:grid-cols-3">
              {details.map(([label, value]) => (
                <view key={label} className="items-center gap-1">
                  <text className="text-sm text-muted">{label}</text>
                  <text data-testid={`location-${label.toLowerCase()}`} className="text-sm font-medium">
                    {value}
                  </text>
                </view>
              ))}
            </view>
          </view>
        ) : (
          <text data-testid="location-status" className={data.locationError ? 'py-8 text-base text-danger' : 'py-8 text-base text-muted'}>
            {String(data.locationError || t('location.none'))}
          </text>
        )}
      </Card>
      <Button testId="location-get" disabled={!!data.isLoading} onClick={() => actions.getLocation()}>
        {data.isLoading ? t('location.getting') : t('location.get')}
      </Button>
      <Button variant="secondary" testId="location-clear" disabled={!!data.isLoading} onClick={() => actions.clearLocation()}>
        {t('location.clear')}
      </Button>
    </Page>
  );
}
