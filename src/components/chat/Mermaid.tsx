import React, { useEffect, useRef, useState } from "react";
import mermaid from "mermaid";
import { XIcon, Maximize2Icon } from "lucide-react";
import { createPortal } from "react-dom";

// Initialize mermaid with a dark-mode friendly theme by default
mermaid.initialize({
  startOnLoad: false,
  theme: "dark",
  securityLevel: "loose",
  fontFamily: "inherit",
});

interface MermaidProps {
  chart: string;
}

export const Mermaid: React.FC<MermaidProps> = ({ chart }) => {
  const [svg, setSvg] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let isMounted = true;

    const renderChart = async () => {
      if (!chart.trim()) return;

      try {
        // Ensure mermaid is initialized before rendering
        await mermaid.run();

        const id = `mermaid-${Math.random().toString(36).substring(2, 11)}`;
        const { svg: rawSvg } = await mermaid.render(id, chart);

        if (isMounted) {
          // Process SVG to ensure it's responsive
          const responsiveSvg = rawSvg
            .replace(/width="[^"]*"/, 'width="100%"')
            .replace(/height="[^"]*"/, 'height="auto"')
            .replace(
              /style="[^"]*max-width:[^;]*;?"/,
              'style="max-width: 100%;"',
            );

          setSvg(responsiveSvg);
          setError(null);
        }
      } catch (err) {
        console.error("Mermaid rendering error:", err);
        if (isMounted) {
          setError("Failed to render Mermaid diagram");
        }
      }
    };

    renderChart();
    return () => {
      isMounted = false;
    };
  }, [chart]);

  // Handle ESC key to close fullscreen
  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") setIsFullscreen(false);
    };
    if (isFullscreen) {
      window.addEventListener("keydown", handleEsc);
    }
    return () => window.removeEventListener("keydown", handleEsc);
  }, [isFullscreen]);

  if (error) {
    return (
      <div className="my-4 rounded-lg border border-red-500/50 bg-red-500/10 p-4 text-sm text-red-500">
        <p className="font-semibold">Mermaid Error:</p>
        <pre className="mt-2 overflow-x-auto whitespace-pre-wrap">{chart}</pre>
      </div>
    );
  }

  const FullscreenOverlay = () =>
    createPortal(
      <div
        className="fixed inset-0 z-[9999] flex flex-col items-center justify-center bg-black/90 p-8 backdrop-blur-sm animate-in fade-in duration-200"
        onClick={() => setIsFullscreen(false)}
      >
        <button
          className="absolute right-6 top-6 rounded-full bg-white/10 p-2 text-white hover:bg-white/20 transition-colors"
          onClick={(e) => {
            e.stopPropagation();
            setIsFullscreen(false);
          }}
        >
          <XIcon className="h-6 w-6" />
        </button>
        <div
          className="h-full w-full flex items-center justify-center [&_svg]:max-h-full [&_svg]:max-w-full [&_svg]:w-auto [&_svg]:h-auto"
          onClick={(e) => e.stopPropagation()}
          dangerouslySetInnerHTML={{ __html: svg }}
        />
      </div>,
      document.body,
    );

  return (
    <>
      <div
        ref={containerRef}
        className="group relative my-2 flex cursor-zoom-in justify-center rounded-lg border border-transparent transition-all hover:border-[var(--color-logo-primary)]/30 hover:bg-[var(--color-text)]/5 p-2"
        onClick={() => setIsFullscreen(true)}
      >
        <div
          className="w-full max-w-full overflow-hidden [&_svg]:w-full [&_svg]:h-auto"
          dangerouslySetInnerHTML={{ __html: svg }}
        />
        <div className="absolute right-2 top-2 opacity-0 group-hover:opacity-100 transition-opacity">
          <Maximize2Icon className="h-4 w-4 text-[var(--color-text)]/40" />
        </div>
      </div>
      {isFullscreen && <FullscreenOverlay />}
    </>
  );
};
