import { useEffect, useState } from "react";
import "./App.css";
import { Settings } from "./components/settings/Settings";
import {
  checkAccessibilityPermissions,
  requestAccessibilityPermissions,
} from "tauri-plugin-macos-permissions-api";
import { register } from "@tauri-apps/plugin-global-shortcut";
import HandyTextLogo from "./components/icons/HandyTextLogo";

function App() {
  const [hasAccessibility, setHasAccessibility] = useState(false);

  useEffect(() => {
    checkAccessibilityPermissions().then((hasPermissions) => {
      if (!hasPermissions) {
        requestAccessibilityPermissions().then((permissions) => {
          setHasAccessibility(permissions);
        });
        return;
      }
      setHasAccessibility(hasPermissions);
    });
  }, []);

  return (
    <div className="min-h-screen bg-gray-100 flex flex-col items-center pt-6 w-full">
      {!hasAccessibility && (
        <div className="bg-yellow-50 p-4 w-full">
          <div className="flex">
            <div className="ml-3">
              <p className="text-sm font-medium text-yellow-800">
                Accessibility permissions are required for keyboard shortcuts.
              </p>
            </div>
          </div>
        </div>
      )}
      <HandyTextLogo width={250} />
      <Settings />
    </div>
  );
}

export default App;
