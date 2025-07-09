import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.jsx'

window.useLingXiaData = function () {
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

  return data;
};

// Page functions injection
/* {{PAGE_FUNCTIONS}} */

ReactDOM.createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
