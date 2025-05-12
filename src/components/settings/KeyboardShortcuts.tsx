import React, { useEffect, useState, useRef } from "react";
import { load } from "@tauri-apps/plugin-store";
import {
  BindingResponseSchema,
  SettingsSchema,
  ShortcutBindingSchema,
  ShortcutBindingsMap,
} from "../../lib/types";
import { invoke } from "@tauri-apps/api/core";
import keycode from "keycode";
import ResetIcon from "../icons/ResetIcon";

export const KeyboardShortcuts: React.FC = () => {
  const [bindings, setBindings] = React.useState<ShortcutBindingsMap>({});
  const [pttEnabled, setPttEnabled] = React.useState<boolean>(false);
  const [keyPressed, setKeyPressed] = useState<string[]>([]);
  const [recordedKeys, setRecordedKeys] = useState<string[]>([]);
  const [editingShortcutId, setEditingShortcutId] = useState<string | null>(
    null
  );
  const [originalBinding, setOriginalBinding] = useState<string>("");
  const shortcutRefs = useRef<Map<string, HTMLDivElement | null>>(new Map());

  useEffect(() => {
    load("settings_store.json", { autoSave: false }).then((r) => {
      console.log("loaded store", r);

      r.get("settings").then((s) => {
        const settings = SettingsSchema.parse(s);
        setBindings(settings.bindings);
        setPttEnabled(settings.push_to_talk);
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
          setOriginalBinding("");
        }
      }
    };

    // Add click outside handler
    const handleClickOutside = (e: MouseEvent) => {
      const activeElement = shortcutRefs.current.get(editingShortcutId);
      if (activeElement && !activeElement.contains(e.target as Node)) {
        // Cancel shortcut recording and restore original value
        if (editingShortcutId && bindings[editingShortcutId]) {
          setBindings((prev) => ({
            ...prev,
            [editingShortcutId]: {
              ...prev[editingShortcutId],
              current_binding: originalBinding,
            },
          }));

          // Reset states
          setEditingShortcutId(null);
          setKeyPressed([]);
          setRecordedKeys([]);
          setOriginalBinding("");
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    window.addEventListener("mousedown", handleClickOutside);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
      window.removeEventListener("mousedown", handleClickOutside);
    };
  }, [keyPressed, recordedKeys, editingShortcutId, bindings, originalBinding]);

  // Start recording a new shortcut
  const startRecording = (id: string) => {
    // Store the original binding to restore if canceled
    setOriginalBinding(bindings[id]?.current_binding || "");
    setEditingShortcutId(id);
    setKeyPressed([]);
    setRecordedKeys([]);
  };

  // Format the current shortcut keys being recorded
  const formatCurrentKeys = () => {
    return recordedKeys.length > 0 ? recordedKeys.join("+") : "Press keys...";
  };

  // Store references to shortcut elements
  const setShortcutRef = (id: string, ref: HTMLDivElement | null) => {
    shortcutRefs.current.set(id, ref);
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between p-4 rounded-lg border border-mid-gray/20 ">
        <div className="max-w-2/3">
          <h3 className="text-sm font-medium ">Push To Talk</h3>
          <p className="text-sm">Hold to record, release to stop</p>
        </div>
        <label className="inline-flex items-center cursor-pointer">
          <input
            type="checkbox"
            value=""
            className="sr-only peer"
            checked={pttEnabled}
            onChange={(e) => {
              console.log("change ptt setting", e.target.checked);
              const newValue = e.target.checked;
              setPttEnabled(newValue);

              invoke("change_ptt_setting", {
                enabled: newValue,
              });
            }}
          />
          <div className="relative w-11 h-6 bg-mid-gray/20 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-logo-primary rounded-full peer peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-logo-primary"></div>
        </label>
      </div>
      {Object.entries(bindings).map(([id, binding]) => (
        <div
          key={id}
          className="flex items-center justify-between p-4 rounded-lg border border-mid-gray/20 "
        >
          <div>
            <h3 className="text-sm font-medium ">{binding.name}</h3>
            <p className="text-sm">{binding.description}</p>
          </div>
          <div className="flex items-center space-x-1">
            {editingShortcutId === id ? (
              <div
                ref={(ref) => setShortcutRef(id, ref)}
                className="px-2 py-1 text-sm font-semibold border border-logo-primary bg-logo-primary/30 rounded min-w-[100px] text-center"
              >
                {formatCurrentKeys()}
              </div>
            ) : (
              <div
                className="px-2 py-1 text-sm font-semibold bg-mid-gray/10 border  border-mid-gray/80 hover:bg-logo-primary/10 rounded cursor-pointer hover:border-logo-primary"
                onClick={() => startRecording(id)}
              >
                {binding.current_binding}
              </div>
            )}
            <button
              className="px-2 py-1 hover:bg-logo-primary/30 active:bg-logo-primary/50 active:scale-95 rounded fill-text hover:cursor-pointer hover:border-logo-primary border border-transparent transition-all duration-150"
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
              <ResetIcon className="" />
            </button>
          </div>
        </div>
      ))}
    </div>
  );
};
