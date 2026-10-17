import React from 'react';
import '../../tailwind.css';

export default function AboutPage() {
  return (
    <div className="min-h-screen bg-gray-100 px-5 py-6">
      <div className="rounded-2xl bg-white shadow-sm p-5">
        <p className="text-base font-semibold text-gray-800">LingXia Chat</p>
        <p className="text-sm text-gray-500 mt-2">
          A pushed page on top of the chat page. Back returns to the chat.
        </p>
      </div>
    </div>
  );
}
