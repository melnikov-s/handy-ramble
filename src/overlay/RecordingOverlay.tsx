import React, { useEffect } from "react";
import "./RecordingOverlay.css";
import { resolveResource } from "@tauri-apps/api/path";

const RecordingOverlay: React.FC = () => {
  useEffect(() => {
    resolveResource("tray_idle.png").then((r) => {
      console.log("res", r);
    });
  }, []);

  return (
    <div className="recording-overlay">
      <div className="bars-container">
        {Array.from({ length: 8 }, (_, i) => (
          <div
            key={i}
            className="bar"
            style={{
              animationDelay: `${i * 100}ms`,
              animationDuration: `${800 + Math.random() * 400}ms`,
            }}
          />
        ))}
      </div>
    </div>
  );
};

export default RecordingOverlay;
