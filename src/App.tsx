import { useEffect, useState } from "react";
import "./App.css";
import { Settings } from "./components/settings/Settings";
import {
  checkAccessibilityPermissions,
  requestAccessibilityPermissions,
} from "tauri-plugin-macos-permissions-api";
import HandyTextLogo from "./components/icons/HandyTextLogo";

function App() {
  const [hasAccessibility, setHasAccessibility] = useState(false);

  const checkPermissions = () => {
    checkAccessibilityPermissions().then((hasPermissions) => {
      if (!hasPermissions) {
        requestAccessibilityPermissions().then((permissions) => {
          setHasAccessibility(permissions);
        });
        return;
      }
      setHasAccessibility(hasPermissions);
    });
  };

  useEffect(() => {
    checkPermissions();
  }, []);

  return (
    <div className="min-h-screen flex flex-col items-center pt-6 w-full">
      {!hasAccessibility && (
        <div className="bg-yellow-50 p-4 w-full">
          <div className="flex justify-between items-center">
            <div className="ml-3">
              <p className="text-sm font-medium">
                Accessibility permissions are required for keyboard shortcuts.
              </p>
            </div>
            <button
              onClick={checkPermissions}
              className="bg-yellow-200 hover:bg-yellow-300 font-medium py-1 px-3 rounded text-sm mr-4"
            >
              Recheck Permissions
            </button>
          </div>
        </div>
      )}
      <HandyTextLogo width={300} />
      <Settings />
    </div>
  );
}

export default App;
