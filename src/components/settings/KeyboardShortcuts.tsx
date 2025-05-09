import React, { useEffect, useState } from "react";
import { load } from "@tauri-apps/plugin-store";
import {
  BindingResponseSchema,
  SettingsSchema,
  ShortcutBindingSchema,
  ShortcutBindingsMap,
} from "../../lib/types";
import { invoke } from "@tauri-apps/api/core";
import keycode from "keycode";

export const KeyboardShortcuts: React.FC = () => {
  const [bindings, setBindings] = React.useState<ShortcutBindingsMap>({});
  const [keyPressed, setKeyPressed] = useState<string[]>([]);
  const [recordedKeys, setRecordedKeys] = useState<string[]>([]);
  const [editingShortcutId, setEditingShortcutId] = useState<string | null>(
    null
  );

  useEffect(() => {
    load("settings_store.json", { autoSave: false }).then((r) => {
      console.log("loaded store", r);

      r.get("settings").then((s) => {
        const settings = SettingsSchema.parse(s);
        setBindings(settings.bindings);
      });
    });
  }, []);

  useEffect(() => {
    // Only add event listeners when we're in editing mode
    if (editingShortcutId === null) return;

    console.log("keyPressed", keyPressed);

    // Keyboard event listeners
    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();

      const key = keycode(e).toLowerCase();
      console.log("You pressed", key);

      if (!keyPressed.includes(key)) {
        setKeyPressed((prev) => [...prev, key]);
        // Also add to recorded keys if not already there
        if (!recordedKeys.includes(key)) {
          setRecordedKeys((prev) => [...prev, key]);
        }
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      e.preventDefault();

      const key = keycode(e).toLowerCase();

      // Remove from currently pressed keys
      setKeyPressed((prev) => prev.filter((k) => k !== key));

      // If no keys are pressed anymore, commit the shortcut
      if (keyPressed.length === 1 && keyPressed[0] === key) {
        // Create the shortcut string from all recorded keys
        const newShortcut = recordedKeys.join("+");

        if (editingShortcutId && bindings[editingShortcutId]) {
          const updatedBinding = {
            ...bindings[editingShortcutId],
            current_binding: newShortcut,
          };

          setBindings((prev) => ({
            ...prev,
            [editingShortcutId]: updatedBinding,
          }));

          invoke("change_binding", {
            id: editingShortcutId,
            binding: newShortcut,
          });

          // Exit editing mode and reset states
          setEditingShortcutId(null);
          setKeyPressed([]);
          setRecordedKeys([]);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, [keyPressed, recordedKeys, editingShortcutId, bindings]);

  // Start recording a new shortcut
  const startRecording = (id: string) => {
    setEditingShortcutId(id);
    setKeyPressed([]);
    setRecordedKeys([]);
  };

  // Format the current shortcut keys being recorded
  const formatCurrentKeys = () => {
    return recordedKeys.length > 0 ? recordedKeys.join("+") : "Press keys...";
  };

  return (
    <div className="space-y-4">
      {Object.entries(bindings).map(([id, binding]) => (
        <div
          key={id}
          className="flex items-center justify-between p-4 rounded-lg border border-gray-200 hover:bg-gray-50"
        >
          <div>
            <h3 className="text-sm font-medium text-gray-900">
              {binding.name}
            </h3>
            <p className="text-sm text-gray-500">{binding.description}</p>
          </div>
          <div className="flex items-center space-x-1">
            {editingShortcutId === id ? (
              <div className="px-2 py-1 text-sm font-semibold text-blue-600 bg-blue-50 border border-blue-200 rounded min-w-[100px] text-center">
                {formatCurrentKeys()}
              </div>
            ) : (
              <div
                className="px-2 py-1 text-sm font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded cursor-pointer hover:bg-gray-200"
                onClick={() => startRecording(id)}
              >
                {binding.current_binding}
              </div>
            )}
            <button
              className="px-2 py-1 text-sm font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded hover:bg-gray-50"
              onClick={() => {
                invoke("reset_binding", { id }).then((b) => {
                  console.log("reset");
                  const newBinding = BindingResponseSchema.parse(b);

                  if (!newBinding.success) {
                    console.error("Error resetting binding:", newBinding.error);
                    return;
                  }

                  const binding = newBinding.binding!;

                  setBindings({ ...bindings, [binding.id]: binding });
                });
              }}
            >
              reset
            </button>
          </div>
        </div>
      ))}
    </div>
  );
};
