import React from 'react';
import '../../tailwind.css';
import './index.css';

type PageData = {
  greeting?: string;
  imageUrl?: string;
  ipAddr?: string;
};

type PageActions = {
  data: PageData;
  greet(payload: { name: string }): void;
};

declare function useLingXia(): PageActions;

export default function HomePage() {
  const { data, greet } = useLingXia();
  const [name, setName] = React.useState('');
  const [isSending, setIsSending] = React.useState(false);

  const greetingMessage = typeof data?.greeting === 'string' ? data.greeting : '';
  const ipAddress = typeof data?.ipAddr === 'string' ? data.ipAddr : '';
  const imageUrl = typeof data?.imageUrl === 'string' ? data.imageUrl : '';

  React.useEffect(() => {
    if (isSending && greetingMessage) {
      setIsSending(false);
    }
  }, [greetingMessage, isSending]);

  const handleGreet = React.useCallback(() => {
    const trimmed = name.trim();
    if (!trimmed) {
      return;
    }
    setIsSending(true);
    greet({ name: trimmed });
  }, [name, greet]);

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      handleGreet();
    }
  };

  return (
    <div className="home-page">
      {imageUrl && (
        <img
          src={imageUrl}
          alt=""
          className="background-image"
        />
      )}
      <div className="container">
        <h1>Welcome to LingXia</h1>

        <div className="input-group">
          <input
            type="text"
            placeholder="Enter a name"
            value={name}
            onChange={event => setName(event.target.value)}
            onKeyDown={handleKeyDown}
          />
          <button type="button" onClick={handleGreet} disabled={isSending}>
            {isSending ? 'Greeting...' : 'Greet'}
          </button>
        </div>

        <div className={`result ${greetingMessage ? '' : 'hidden'}`}>
          {greetingMessage}
        </div>

        <div className="ip-info">
          {ipAddress ? `Public IP: ${ipAddress}` : ''}
        </div>
      </div>
    </div>
  );
}
