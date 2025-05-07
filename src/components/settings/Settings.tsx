import React from "react";
// import { AIConfig } from "./AIConfig";
import { KeyboardShortcuts } from "./KeyboardShortcuts";

export const Settings: React.FC = () => {
  return (
    <div className="bg-gray-100 py-4 px-4 w-full">
      <div className="max-w-3xl mx-auto space-y-4">
        <div className="">
          <h2 className="text-xl font-semibold text-gray-900 mb-2">
            Keyboard Shortcuts
          </h2>
          <KeyboardShortcuts />
        </div>
      </div>
    </div>
  );
};
