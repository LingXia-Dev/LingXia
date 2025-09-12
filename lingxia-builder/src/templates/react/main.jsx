import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.jsx'

window.useLingXia = function () {
  const [data, setData] = React.useState({});

  React.useEffect(() => {
    if (window.LingXiaBridge && window.LingXiaBridge.subscribe) {
      window.LingXiaBridge.subscribe((newData) => {
        if (newData) {
          setData(prevData => ({ ...prevData, ...newData }));
        }
      });
    }
  }, []);

  // Create functions object from page functions
  const functions = React.useMemo(() => {
    if (!window.__PAGE_FUNCTIONS) return {};

    return window.__PAGE_FUNCTIONS.reduce((acc, funcName) => {
      acc[funcName] = window[funcName];
      return acc;
    }, {});
  }, []);

  // Return both data and functions
  return {
    data,
    ...functions
  };
};

// Page functions injection
/* {{PAGE_FUNCTIONS}} */

ReactDOM.createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
