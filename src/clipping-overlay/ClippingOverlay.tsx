import React, { useState, useEffect, useRef } from "react";
import { commands } from "@/bindings";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen } from "@tauri-apps/api/event";

export const ClippingOverlay: React.FC = () => {
  const [startPos, setStartPos] = useState<{ x: number; y: number } | null>(
    null,
  );
  const [currentPos, setCurrentPos] = useState<{ x: number; y: number } | null>(
    null,
  );
  const [isSelecting, setIsSelecting] = useState(false);
  const [isCapturing, setIsCapturing] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const handleCapture = async (
    start: { x: number; y: number },
    end: { x: number; y: number },
  ) => {
    if (isCapturing) return;

    const x = Math.min(start.x, end.x);
    const y = Math.min(start.y, end.y);
    const width = Math.abs(start.x - end.x);
    const height = Math.abs(start.y - end.y);

    // If selection is too small, just cancel
    if (width <= 5 || height <= 5) {
      try {
        await commands.restoreAppVisibility();
      } catch {}
      try {
        await getCurrentWebviewWindow().close();
      } catch {}
      return;
    }

    setIsCapturing(true);

    try {
      console.log(`Capturing region: ${width}x${height} at (${x}, ${y})`);
      const result = await commands.captureRegionCommand(
        Math.round(x),
        Math.round(y),
        Math.round(width),
        Math.round(height),
      );
      console.log("Capture result:", result.status);
    } catch (err) {
      console.error("Failed to capture region:", err);
      // Restore visibility on error
      try {
        await commands.restoreAppVisibility();
      } catch {}
    } finally {
      // ALWAYS reset state and close window
      setIsCapturing(false);
      try {
        await getCurrentWebviewWindow().close();
      } catch (e) {
        console.error("Failed to close clipping window:", e);
      }
    }
  };

  // Reset state function
  const resetState = () => {
    setStartPos(null);
    setCurrentPos(null);
    setIsSelecting(false);
    setIsCapturing(false);
  };

  // Reset state when window gains focus (more reliable than events)
  useEffect(() => {
    const win = getCurrentWebviewWindow();

    // Reset state on component mount
    resetState();

    // Set up focus listener to reset state when window is shown
    const unlistenFocusPromise = win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        console.log("ClippingOverlay: Window focused, resetting state");
        resetState();
      }
    });

    // Also listen for the reset event as a fallback
    const unlistenResetPromise = listen("reset-clipping-state", () => {
      console.log("ClippingOverlay: Reset event received");
      resetState();
    });

    return () => {
      unlistenFocusPromise.then((unlisten) => unlisten());
      unlistenResetPromise.then((unlisten) => unlisten());
    };
  }, []);

  // Escape key handler
  useEffect(() => {
    const win = getCurrentWebviewWindow();

    const handleKeyDown = async (e: KeyboardEvent) => {
      if (e.key === "Escape" && !isCapturing) {
        console.log("ClippingOverlay: Escape pressed, canceling");
        await commands.restoreAppVisibility();
        await win.close();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isCapturing]);

  useEffect(() => {
    if (!isSelecting || isCapturing) return;

    const onMouseMove = (e: MouseEvent) => {
      // Use screenX/Y for global coordinates
      setCurrentPos({ x: e.screenX, y: e.screenY });
    };

    const onMouseUp = (e: MouseEvent) => {
      setIsSelecting(false);
      const lastPos = { x: e.screenX, y: e.screenY };
      if (startPos) {
        handleCapture(startPos, lastPos);
      } else {
        commands
          .restoreAppVisibility()
          .then(() => getCurrentWebviewWindow().close());
      }
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);

    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [isSelecting, isCapturing, startPos]);

  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0 || isCapturing) return; // Only left click and not while capturing
    // Use screenX/Y for global coordinates
    setStartPos({ x: e.screenX, y: e.screenY });
    setCurrentPos({ x: e.screenX, y: e.screenY });
    setIsSelecting(true);
  };

  const selectionRect =
    startPos && currentPos
      ? {
          left: Math.min(startPos.x, currentPos.x),
          top: Math.min(startPos.y, currentPos.y),
          width: Math.abs(startPos.x - currentPos.x),
          height: Math.abs(startPos.y - currentPos.y),
        }
      : null;

  return (
    <div
      ref={containerRef}
      className={`h-full w-full select-none ${isCapturing ? "cursor-wait bg-black/20" : "cursor-crosshair bg-transparent"}`}
      onMouseDown={handleMouseDown}
    >
      {!isCapturing ? (
        <div className="pointer-events-none absolute left-4 top-4 rounded bg-black/70 px-4 py-2 text-sm text-white shadow-2xl backdrop-blur-md border border-white/20 select-none">
          <p className="font-medium text-blue-400">Regional Clip Mode</p>
          <p className="text-xs opacity-90 mt-1">
            Drag to select a region. Release to capture. Esc to cancel.
          </p>
        </div>
      ) : (
        <div className="absolute inset-0 flex items-center justify-center">
          <div className="rounded-lg bg-black/80 px-8 py-4 text-white shadow-2xl backdrop-blur-xl border border-white/10 flex flex-col items-center gap-3">
            <div className="h-6 w-6 animate-spin rounded-full border-2 border-blue-500 border-t-transparent" />
            <p className="font-semibold text-lg tracking-wide text-blue-400">
              Capturing...
            </p>
          </div>
        </div>
      )}

      {selectionRect && !isCapturing && (
        <div
          className="pointer-events-none absolute border-2 border-blue-500 bg-blue-500/5 shadow-[0_0_0_9999px_rgba(0,0,0,0.5)]"
          style={{
            left: selectionRect.left,
            top: selectionRect.top,
            width: selectionRect.width,
            height: selectionRect.height,
          }}
        >
          <div className="absolute -bottom-8 right-0">
            <div className="rounded bg-blue-600 px-2 py-1 text-[10px] font-bold text-white shadow-lg">
              {Math.round(selectionRect.width)} ×{" "}
              {Math.round(selectionRect.height)}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
