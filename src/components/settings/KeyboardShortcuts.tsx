import React from "react";

interface Shortcut {
  id: string;
  name: string;
  description: string;
  keys: string[];
}

export const KeyboardShortcuts: React.FC = () => {
  // These would normally come from the backend configuration
  const shortcuts: Shortcut[] = [
    {
      id: "transcribe",
      name: "Transcribe",
      description: "Convert speech to text",
      keys: ["⌃", "⌘"],
    },
    // {
    //   id: 'instruct',
    //   name: 'AI Chat',
    //   description: 'Start a conversation with AI',
    //   keys: ['⇧', '⌥'],
    // },
    // {
    //   id: 'code',
    //   name: 'Generate Code',
    //   description: 'Generate code from voice input',
    //   keys: ['⌃', '⌥', '⌘'],
    // },
  ];

  return (
    <div className="space-y-4">
      {shortcuts.map((shortcut) => (
        <div
          key={shortcut.id}
          className="flex items-center justify-between p-4 rounded-lg border border-gray-200 hover:bg-gray-50"
        >
          <div>
            <h3 className="text-sm font-medium text-gray-900">
              {shortcut.name}
            </h3>
            <p className="text-sm text-gray-500">{shortcut.description}</p>
          </div>
          <div className="flex items-center space-x-1">
            {shortcut.keys.map((key, index) => (
              <React.Fragment key={index}>
                <kbd className="px-2 py-1 text-sm font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded">
                  {key}
                </kbd>
                {index < shortcut.keys.length - 1 && (
                  <span className="text-gray-500">+</span>
                )}
              </React.Fragment>
            ))}
          </div>
        </div>
      ))}

      <div className="mt-4 rounded-md bg-yellow-50 p-4">
        <div className="flex">
          <div className="ml-3">
            <p className="text-sm text-yellow-700">
              Note: Keyboard shortcuts are currently managed by the system.
              Custom configuration will be available in a future update.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};
