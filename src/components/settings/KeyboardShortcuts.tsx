import React, { useEffect } from "react";
import { load } from "@tauri-apps/plugin-store";
import { SettingsSchema, ShortcutBinding } from "../../lib/types";

export const KeyboardShortcuts: React.FC = () => {
  const [bindings, setBindings] = React.useState<ShortcutBinding[]>([]);

  useEffect(() => {
    load("settings_store.json", { autoSave: false }).then((r) => {
      console.log("loaded store", r);

      r.get("settings").then((s) => {
        const settings = SettingsSchema.parse(s);
        setBindings(settings.bindings);
      });
    });
  }, []);

  return (
    <div className="space-y-4">
      {bindings.map((binding) => (
        <div
          key={binding.id}
          className="flex items-center justify-between p-4 rounded-lg border border-gray-200 hover:bg-gray-50"
        >
          <div>
            <h3 className="text-sm font-medium text-gray-900">
              {binding.name}
            </h3>
            <p className="text-sm text-gray-500">{binding.description}</p>
          </div>
          <div className="flex items-center space-x-1">
            <React.Fragment>
              {/* <kbd className="px-2 py-1 text-sm font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded"> */}
              <div className="px-2 py-1 text-sm font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded">
                {binding.current_binding}
              </div>
              {/* </kbd> */}
            </React.Fragment>
          </div>
        </div>
      ))}
    </div>
  );
};
