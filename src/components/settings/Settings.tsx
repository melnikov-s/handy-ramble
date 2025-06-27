import React from "react";
// import { AIConfig } from "./AIConfig";
import { KeyboardShortcuts } from "./KeyboardShortcuts";

export const Settings: React.FC = () => {
  return (
    <div className="max-w-3xl w-full mx-auto space-y-4">
      <KeyboardShortcuts />
    </div>
  );
};
