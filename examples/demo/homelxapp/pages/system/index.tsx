import React from 'react';
import '../../tailwind.css';

export default function SystemPage() {
  const { data, getAppBaseInfo } = useLingXia();
  const { currentType = 'appBaseInfo', appBaseInfo = null } = data;

  React.useEffect(() => {
    // Reset display when switching type (future-proof for multiple demos)
  }, [currentType]);

  return (
    <div className="min-h-screen bg-gray-100">
      <div className="max-w-md mx-auto pb-10">
        {currentType === 'appBaseInfo' && (
          <>
            <div className="mt-6 mb-3 px-5 text-sm text-gray-500 font-medium">App Base Info</div>
            <div className="mx-3 mb-4 bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
              <div className="flex items-center px-4 py-4">
                <div className="text-xl mr-4">🧭</div>
                <div className="flex-1">
                  <div className="text-base text-black mb-0.5 font-medium">Fetch App Base Info</div>
                </div>
                <button
                  onClick={getAppBaseInfo}
                  className="h-7 px-3 text-xs font-medium transition-all duration-200 bg-blue-500 hover:bg-blue-600 text-white border-0 shadow-sm rounded"
                >
                  Get
                </button>
              </div>

              {appBaseInfo && (
                <div className="mx-4 mb-4 p-4 bg-gray-50 rounded-lg border border-gray-200">
                  <h4 className="text-sm font-medium text-gray-700 mb-3">Result</h4>
                  <InfoRow label="Language" value={appBaseInfo.language} />
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

interface InfoRowProps {
  label: string;
  value?: string;
}

function InfoRow({ label, value }: InfoRowProps) {
  const display = value || '-';
  return (
    <div className="flex justify-between items-center py-2 border-b border-gray-100 last:border-b-0">
      <span className="text-sm text-gray-600">{label}</span>
      <span className="text-sm font-medium text-gray-900">{display}</span>
    </div>
  );
}
