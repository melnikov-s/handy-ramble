import React from 'react';
import './RecordingOverlay.css';

const RecordingOverlay: React.FC = () => {
  return (
    <div className="recording-overlay">
      <div className="bars-container">
        {Array.from({ length: 8 }, (_, i) => (
          <div
            key={i}
            className="bar"
            style={{
              animationDelay: `${i * 100}ms`,
              animationDuration: `${800 + Math.random() * 400}ms`
            }}
          />
        ))}
      </div>
    </div>
  );
};

export default RecordingOverlay;
