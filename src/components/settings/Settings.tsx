import React from "react";
// import { AIConfig } from "./AIConfig";
import { KeyboardShortcuts } from "./KeyboardShortcuts";

export const Settings: React.FC = () => {
  return (
    <div className="max-w-3xl w-full mx-auto space-y-4">
      <div className="mt-8">
        {/* <h2 className="text-xl font-semibold mb-2">Keyboard Shortcuts</h2> */}
        <KeyboardShortcuts />
      </div>
    </div>
  );
};
