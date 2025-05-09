import { useEffect, useState } from "react";
import "./App.css";
import { Settings } from "./components/settings/Settings";
import {
  checkAccessibilityPermissions,
  requestAccessibilityPermissions,
} from "tauri-plugin-macos-permissions-api";
import { register } from "@tauri-apps/plugin-global-shortcut";
import HandyTextLogo from "./components/icons/HandyTextLogo";
import { invoke } from "@tauri-apps/api/core";
// import { TrayIcon } from "@tauri-apps/api/tray";
// import { Menu } from "@tauri-apps/api/menu";
// import { defaultWindowIcon } from "@tauri-apps/api/app";

// const menu = await Menu.new({
//   items: [
//     {
//       id: "quit",
//       text: "Quit",
//     },
//   ],
// });

// const options = {
//   icon: await defaultWindowIcon(),
//   menu,
//   menuOnLeftClick: true,
// };

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

    // TrayIcon.new(options).then((t) => {
    //   console.log("tray init", t);
    // });

    // invoke("set_binding", {
    //   id: "test",
    //   binding: "alt+d",
    // });
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
