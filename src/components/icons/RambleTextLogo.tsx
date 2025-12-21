import React from "react";

const RambleTextLogo = ({
  width,
  height,
  className,
}: {
  width?: number;
  height?: number;
  className?: string;
}) => {
  return (
    <svg
      width={width}
      height={height}
      className={className}
      viewBox="0 0 160 40"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <text
        x="0"
        y="30"
        fill="currentColor"
        style={{
          fontFamily: "Inter, system-ui, sans-serif",
          fontWeight: "bold",
          fontSize: "32px",
          letterSpacing: "-0.01em",
        }}
        className="logo-primary"
      >
        Ramble
      </text>
    </svg>
  );
};

export default RambleTextLogo;
